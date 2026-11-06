use super::{Backend, parameter::*};
use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock as Content};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::Serialize;
use serde_json::{Value, json};

use crate::budgets::{MAX_PNG_SIDE, check_png, fit_png, json_text};
use crate::headless::batch_reply;
use crate::security::MAX_BATCH_STEPS;
use crate::workspace::authorize_command;
use crate::{AutomationError, BridgeClient, Headless, bad};

#[derive(Clone)]
pub struct MaterialCraftMcp {
    backend: Arc<Backend>,
    tool_router: ToolRouter<Self>,
}

fn ok_json(v: &Value) -> CallToolResult {
    match json_text(v) {
        Ok(text) => CallToolResult::success(vec![Content::text(text)]),
        Err(e) => fail(e),
    }
}

fn fail(e: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![Content::text(e.to_string())])
}

fn to_result(r: Result<Value, AutomationError>) -> Result<CallToolResult, McpError> {
    Ok(match r {
        Ok(v) => ok_json(&v),
        Err(e) => fail(e),
    })
}

/// A `{width, height, png: base64}` reply (render or app screenshot) as image content
fn image_result(r: Result<Value, AutomationError>, note: &str, refit: bool) -> Result<CallToolResult, McpError> {
    let png = r.and_then(|v| {
        let b64 = v.get("png").and_then(Value::as_str).ok_or_else(|| AutomationError::Other("the reply has no PNG".into()))?;
        if !refit {
            check_png(b64.len() / 4 * 3)?;
            return Ok(b64.to_string());
        }
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| AutomationError::Other(format!("bad PNG data: {e}")))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(fit_png(bytes, MAX_PNG_SIDE)?))
    });
    Ok(match png {
        Ok(b64) => CallToolResult::success(vec![Content::image(b64, "image/png"), Content::text(note.to_string())]),
        Err(e) => fail(e),
    })
}

fn params<T: Serialize>(p: &T) -> Value {
    serde_json::to_value(p).unwrap_or(Value::Null)
}

impl MaterialCraftMcp {
    pub fn headless(h: Headless) -> Self {
        Self::with_backend(Backend::Headless(Arc::new(Mutex::new(h))))
    }

    pub fn bridge(client: BridgeClient) -> Self {
        Self::with_backend(Backend::Bridge(Arc::new(client)))
    }

    pub fn with_backend(backend: Backend) -> Self {
        MaterialCraftMcp { backend: Arc::new(backend), tool_router: Self::tool_router() }
    }

    /// Serve MCP on stdin/stdout until the client disconnects
    pub async fn serve_stdio(self) -> Result<(), AutomationError> {
        let running = self.serve(rmcp::transport::stdio()).await.map_err(|e| AutomationError::Other(format!("MCP start: {e}")))?;
        running.waiting().await.map_err(|e| AutomationError::Other(e.to_string()))?;
        Ok(())
    }

    fn is_bridge(&self) -> bool {
        matches!(*self.backend, Backend::Bridge(_))
    }

    /// One control-channel method on the backend
    async fn call(&self, method: &str, p: Value) -> Result<Value, AutomationError> {
        match &*self.backend {
            Backend::Headless(h) => {
                let (h, m) = (h.clone(), method.to_string());
                // Edits run on snapshots (`Session::edit`), so a panicking command leaves the session consistent: keep serving after a poisoned lock
                tokio::task::spawn_blocking(move || h.lock().unwrap_or_else(PoisonError::into_inner).handle(&m, &p))
                    .await
                    .unwrap_or_else(|_| Err(AutomationError::Other(format!("internal error in `{method}`"))))
            }
            Backend::Bridge(b) => match method {
                "command.run" => {
                    let id = p.get("id").and_then(Value::as_str).unwrap_or_default();
                    authorize_command(id, p.get("params").unwrap_or(&Value::Null))?;
                    b.call(method, p).await
                }
                "package.save" if p.get("path").is_none() => b.call("ui.menu.invoke", json!({"id": "file.save"})).await,
                "package.save" | "package.open" | "bitmap.import" | "outputs.export" | "material.export" | "command.batch" | "render" => {
                    Err(bad(format!("`{method}` with paths works in headless mode only: the running app reads and writes its own files (use ui_menu_invoke)")))
                }
                _ => b.call(method, p).await,
            },
        }
    }

    async fn run(&self, id: &str, p: Value) -> Result<Value, AutomationError> {
        self.call("command.run", json!({"id": id, "params": p})).await
    }

    async fn bridge_only(&self, method: &str, p: Value) -> Result<Value, AutomationError> {
        if self.is_bridge() {
            self.call(method, p).await
        } else {
            Err(bad(format!(
                "`{method}` needs the running app: start `materialcraft --control <port>` and run `materialcraft-cli mcp --bridge <port>` (headless renders: view_render)"
            )))
        }
    }
}

// --------------------------------------------------------------------------------------------- Tools

#[tool_router]
impl MaterialCraftMcp {
    #[tool(description = "The whole session as JSON: package, graphs, the open graph's nodes (definition, parameters, base \
        parameters, ports, cook errors, timings, mean values), connections, frames, comments, pins, exposed inputs, views and history.")]
    async fn session_inspect(&self) -> Result<CallToolResult, McpError> {
        to_result(self.call("session.inspect", json!({})).await)
    }

    #[tool(description = "List engine commands: id, label, menu path, shortcut, params doc and whether it can run now.")]
    async fn command_list(&self, Parameters(p): Parameters<FilterParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("command.list", params(&p)).await)
    }

    #[tool(description = "Run an engine command by id with JSON params (see command_list). Runs with defaults, never opens a \
        dialog. Commands given host paths are disabled: use package_open, package_save, bitmap_import and outputs_export.")]
    async fn command_run(&self, Parameters(p): Parameters<RunParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("command.run", params(&p)).await)
    }

    #[tool(description = "Run several commands in order: steps [{id, params}]. Stops at the first error. Returns {completed, \
        results, failed: {index, id, error} | null}; a failed batch is a tool error.")]
    async fn command_batch(&self, Parameters(p): Parameters<BatchParams>) -> Result<CallToolResult, McpError> {
        if p.steps.len() > MAX_BATCH_STEPS {
            return Ok(fail(format!("at most {MAX_BATCH_STEPS} steps per batch")));
        }
        let reply = if self.is_bridge() {
            let mut results = Vec::new();
            let mut failed = None;
            for (i, s) in p.steps.iter().enumerate() {
                match self.call("command.run", params(s)).await {
                    Ok(v) => results.push(v),
                    Err(e) => {
                        failed = Some((i, s.id.as_str(), e.to_string()));
                        break;
                    }
                }
            }
            batch_reply(results, failed)
        } else {
            match self.call("command.batch", params(&p)).await {
                Ok(v) => v,
                Err(e) => return Ok(fail(e)),
            }
        };
        Ok(if reply["failed"].is_null() { ok_json(&reply) } else { fail(json_text(&reply).unwrap_or_else(|e| e.to_string())) })
    }

    #[tool(description = "The node library: every node's id, label, category, input and output ports and parameters (id, \
        default, range, enum options). Filter by text or category.")]
    async fn library_list(&self, Parameters(p): Parameters<LibraryParams>) -> Result<CallToolResult, McpError> {
        to_result(self.run("library.list", params(&p)).await)
    }

    #[tool(description = "Add a new graph to the package from a template and open it. Returns {graph, name}.")]
    async fn graph_new(&self, Parameters(p): Parameters<NewGraphParams>) -> Result<CallToolResult, McpError> {
        to_result(self.run("file.new", params(&p)).await)
    }

    #[tool(description = "Add a node to the open graph (optionally wired from another node's output). Returns {id}.")]
    async fn node_add(&self, Parameters(p): Parameters<NodeAddParams>) -> Result<CallToolResult, McpError> {
        to_result(self.run("graph.node.add", params(&p)).await)
    }

    #[tool(description = "Wire an output port of one node into an input port of another (replacing what fed that input).")]
    async fn node_connect(&self, Parameters(p): Parameters<ConnectParams>) -> Result<CallToolResult, McpError> {
        to_result(self.run("graph.connect", params(&p)).await)
    }

    #[tool(description = "Set one parameter of a node.")]
    async fn node_param(&self, Parameters(p): Parameters<ParamSetParams>) -> Result<CallToolResult, McpError> {
        to_result(self.run("graph.node.param", params(&p)).await)
    }

    #[tool(
        description = "Open an .sbs package (path relative to the read root). Returns the session; `package.warnings` lists what an SBS file holds that MaterialCraft can't read yet (missing nodes are kept as placeholders)."
    )]
    async fn package_open(&self, Parameters(p): Parameters<PathParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("package.open", params(&p)).await)
    }

    #[tool(description = "Save the package (path relative to the write root; omit to save to its own file). Saves native SBS (.sbs).")]
    async fn package_save(&self, Parameters(p): Parameters<SaveParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("package.save", params(&p)).await)
    }

    #[tool(description = "Embed an image (path relative to the read root) as a bitmap resource. Returns its name, for a \
        `bitmap` node's `resource` parameter.")]
    async fn bitmap_import(&self, Parameters(p): Parameters<BitmapParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("bitmap.import", params(&p)).await)
    }

    #[tool(description = "Cook a graph at full size and write every output (base color, normal, roughness, …) into a folder \
        relative to the write root. Returns the written files.")]
    async fn outputs_export(&self, Parameters(p): Parameters<ExportParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("outputs.export", params(&p)).await)
    }

    #[tool(description = "Cook a graph at full size and write it as a portable .mcmat material (cooked maps, exposed inputs, \
        source package) to a path relative to the write root, for TextureCraft. Optional graph, size and exposed input values. \
        Returns {path, bytes}.")]
    async fn material_export(&self, Parameters(p): Parameters<MaterialExportParams>) -> Result<CallToolResult, McpError> {
        to_result(self.call("material.export", params(&p)).await)
    }

    #[tool(description = "Headless: render the material on the 3D view's scene, or a node's output (view 2d), as a PNG up to \
        2048 px a side. In bridge mode use view_screenshot.")]
    async fn view_render(&self, Parameters(p): Parameters<RenderParams>) -> Result<CallToolResult, McpError> {
        let note = format!("render {}", params(&p));
        image_result(self.call("render", params(&p)).await, &note, false)
    }

    #[tool(description = "Bridge mode: PNG of the live app's 3D or 2D view.")]
    async fn view_screenshot(&self, Parameters(p): Parameters<ScreenshotParams>) -> Result<CallToolResult, McpError> {
        image_result(self.bridge_only("view.screenshot", params(&p)).await, "screenshot of an app view", true)
    }

    #[tool(description = "Bridge mode: PNG of the whole app window (downscaled to 2048 px a side at most).")]
    async fn ui_screenshot(&self) -> Result<CallToolResult, McpError> {
        image_result(self.bridge_only("ui.screenshot", json!({})).await, "screenshot of the app window", true)
    }

    #[tool(description = "Bridge mode: the app's UI state (panels, windows, graph view camera, node rectangles on screen, toasts).")]
    async fn ui_inspect(&self) -> Result<CallToolResult, McpError> {
        to_result(self.bridge_only("ui.inspect", json!({})).await)
    }

    #[tool(description = "Bridge mode: activate a menu item by id like a click (may open a window). ui.menu.list lists ids.")]
    async fn ui_menu_invoke(&self, Parameters(p): Parameters<MenuParams>) -> Result<CallToolResult, McpError> {
        to_result(self.bridge_only("ui.menu.invoke", params(&p)).await)
    }

    #[tool(description = "Bridge mode: synthesize mouse, keyboard and text input in the app window (clicks, drags between \
        node ports, scrolling, shortcuts, typing). Positions are window points (see ui_inspect for node rectangles).")]
    async fn ui_input(&self, Parameters(p): Parameters<InputParams>) -> Result<CallToolResult, McpError> {
        to_result(self.bridge_only("ui.input", params(&p)).await)
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "materialcraft",
    instructions = "MaterialCraft procedural material designer. A package holds graphs of nodes; each graph's output nodes \
        (usages baseColor, normal, roughness, metallic, height, ambientOcclusion) make a PBR material. Start with graph_new \
        (templates: empty, pbr, atomics) or package_open, browse nodes with library_list, build with node_add / node_connect \
        / node_param (or command_run/command_batch for anything else: command_list), check your work with session_inspect and \
        view_render (3d, or 2d for one node), then package_save, outputs_export or material_export. Undo: command_run edit.undo."
)]
impl ServerHandler for MaterialCraftMcp {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::Filesystem;

    #[tokio::test(flavor = "multi_thread")]
    async fn a_panic_does_not_wedge_the_session() {
        let mcp = MaterialCraftMcp::headless(Headless::new(materialcraft_engine::sample_session(), Filesystem::Denied));
        let Backend::Headless(h) = &*mcp.backend else { panic!("headless") };
        let h2 = h.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _g = h2.lock().unwrap();
            panic!("boom")
        })
        .await;
        assert!(mcp.call("session.inspect", json!({})).await.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn material_export_tool_writes_through_the_workspace_only() {
        let base = std::env::temp_dir().join(format!("mc-mcp-{}-material", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let args = || Parameters(MaterialExportParams { path: "m".into(), graph: None, size: Some(16), values: None });

        let denied = MaterialCraftMcp::headless(Headless::new(materialcraft_engine::sample_session(), Filesystem::Denied));
        assert_eq!(denied.material_export(args()).await.unwrap().is_error, Some(true));

        let ws = crate::AuthorizedWorkspace::new(None, Some(&base)).unwrap();
        let mcp = MaterialCraftMcp::headless(Headless::new(materialcraft_engine::sample_session(), Filesystem::Workspace(ws)));
        assert_eq!(mcp.material_export(args()).await.unwrap().is_error, Some(false));
        assert!(base.join("m.mcmat").is_file());
        let escape = Parameters(MaterialExportParams { path: "../m.mcmat".into(), graph: None, size: Some(16), values: None });
        assert_eq!(mcp.material_export(escape).await.unwrap().is_error, Some(true));
        assert!(!base.parent().unwrap().join("m.mcmat").exists());
    }

    #[test]
    fn oversized_replies_become_errors() {
        let r = ok_json(&json!("x".repeat(crate::budgets::MAX_RESPONSE_BYTES)));
        assert_eq!(r.is_error, Some(true));
    }
}
