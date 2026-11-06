pub(super) struct FilterMap {
    pub(super) sbs: &'static str,
    pub(super) def: &'static str,
    pub(super) inputs: &'static [(&'static str, &'static str)],
    pub(super) params: &'static [(&'static str, &'static str)],
}

pub(super) const FILTERS: &[FilterMap] = &[
    FilterMap { sbs: "motionblur", def: "motion_blur", inputs: &[("input1", "input")], params: &[("intensity", "intensity"), ("shiftangle", "shiftangle")] },
    FilterMap { sbs: "uniform", def: "uniform", inputs: &[], params: &[("outputcolor", "color"), ("colorswitch", "color_mode")] },
    FilterMap {
        sbs: "blend",
        def: "blend",
        inputs: &[("source", "foreground"), ("destination", "background"), ("opacity", "mask")],
        params: &[("blendingmode", "mode"), ("opacitymult", "opacity"), ("alphablending", "alpha")],
    },
    FilterMap { sbs: "blur", def: "blur", inputs: &[("input1", "input")], params: &[("intensity", "intensity")] },
    FilterMap {
        sbs: "shuffle",
        def: "channels_shuffle",
        inputs: &[("input1", "input1"), ("input2", "input2")],
        params: &[("channelred", "red"), ("channelgreen", "green"), ("channelblue", "blue"), ("channelalpha", "alpha")],
    },
    FilterMap { sbs: "curve", def: "curve", inputs: &[("input1", "input")], params: &[] },
    FilterMap { sbs: "dirmotionblur", def: "directional_blur", inputs: &[("input1", "input")], params: &[("intensity", "intensity"), ("mblurangle", "angle")] },
    FilterMap {
        sbs: "directionalwarp",
        def: "directional_warp",
        inputs: &[("input1", "input"), ("inputintensity", "intensity_input")],
        params: &[("intensity", "intensity"), ("warpangle", "angle")],
    },
    FilterMap {
        sbs: "distance",
        def: "distance",
        inputs: &[("mask", "mask"), ("source", "source")],
        params: &[("distance", "distance"), ("combinedistance", "combine")],
    },
    FilterMap {
        sbs: "emboss",
        def: "emboss",
        inputs: &[("input1", "input"), ("inputgradient", "height")],
        params: &[("intensity", "intensity"), ("lightangle", "light_angle"), ("highlightcolor", "highlight"), ("shadowcolor", "shadow")],
    },
    FilterMap {
        sbs: "dyngradient",
        def: "gradient_dynamic",
        inputs: &[("input1", "input"), ("input2", "gradient"), ("inputgradient", "gradient")],
        params: &[("position", "position"), ("orientation", "orientation")],
    },
    FilterMap { sbs: "gradient", def: "gradient_map", inputs: &[("input1", "input")], params: &[("colorswitch", "color_mode")] },
    FilterMap {
        sbs: "grayscaleconversion",
        def: "grayscale_conversion",
        inputs: &[("input1", "input")],
        params: &[("alphamult", "flatten_alpha"), ("mattelevel", "matte")],
    },
    FilterMap { sbs: "hsl", def: "hsl", inputs: &[("input1", "input")], params: &[("hue", "hue"), ("saturation", "saturation"), ("luminance", "lightness")] },
    FilterMap { sbs: "bitmap", def: "bitmap", inputs: &[], params: &[("colorswitch", "color_mode"), ("colorspace", "colour_space")] },
    FilterMap {
        sbs: "levels",
        def: "levels",
        inputs: &[("input1", "input")],
        params: &[("levelinlow", "in_low"), ("levelinmid", "in_mid"), ("levelinhigh", "in_high"), ("leveloutlow", "out_low"), ("levelouthigh", "out_high")],
    },
    FilterMap {
        sbs: "normal",
        def: "normal",
        inputs: &[("input1", "input")],
        params: &[("intensity", "intensity"), ("inversedy", "opengl"), ("input2alpha", "height_alpha")],
    },
    FilterMap {
        sbs: "pixelprocessor",
        def: "pixel_processor",
        inputs: &[("input", "input0")],
        params: &[("perpixel", "function"), ("colorswitch", "color_mode")],
    },
    FilterMap { sbs: "valueprocessor", def: "value_processor", inputs: &[("input", "input0")], params: &[("function", "function")] },
    FilterMap { sbs: "sharpen", def: "sharpen", inputs: &[("input1", "input")], params: &[("intensity", "intensity")] },
    FilterMap {
        sbs: "transformation",
        def: "transformation",
        inputs: &[("input1", "input")],
        params: &[("offset", "offset"), ("mattecolor", "matte"), ("matrix22", "matrix"), ("filtering", "filtering")],
    },
    FilterMap { sbs: "warp", def: "warp", inputs: &[("input1", "input"), ("inputgradient", "gradient")], params: &[("intensity", "intensity")] },
    FilterMap { sbs: "passthrough", def: "dot", inputs: &[("input", "input"), ("input1", "input")], params: &[] },
    FilterMap {
        sbs: "text",
        def: "text",
        inputs: &[("background", "background")],
        params: &[
            ("colorswitch", "color_mode"),
            ("background", "background"),
            ("text", "text"),
            ("fontdata", "fontdata"),
            ("fontsize", "fontsize"),
            ("alignment", "alignment"),
            ("position", "position"),
            ("matrix22", "matrix"),
        ],
    },
    FilterMap { sbs: "svg", def: "svg", inputs: &[("background", "background")], params: &[("colorswitch", "color_mode"), ("background", "background")] },
    FilterMap {
        sbs: "fxmaps",
        def: "fx_map",
        inputs: &[("background", "background"), ("input", "pattern0")],
        params: &[("colorswitch", "color_mode"), ("background", "background"), ("opacitymult", "opacity")],
    },
];
pub(super) const BASE_PARAMS: &[&str] = &["outputsize", "format", "randomseed", "tiling", "pixelsize", "pixelratio", "quality"];
pub(super) const HANDLED_PARAMS: &[(&str, &str)] =
    &[("bitmap", "bitmapresourcepath"), ("grayscaleconversion", "channelsweights"), ("transformation", "matrix22"), ("curve", "exposecurve")];

pub(super) fn filter_by_sbs_name(name: &str) -> Option<&'static FilterMap> {
    FILTERS.iter().find(|f| f.sbs == name)
}

pub(super) fn filter_by_def(def: &str) -> Option<&'static FilterMap> {
    FILTERS.iter().find(|f| f.def == def)
}
