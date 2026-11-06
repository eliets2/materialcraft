use super::value_type;
use materialcraft_core::Value;

pub(super) fn inferred_type(op: &str, value: Option<&Value>, input: impl Fn(usize) -> u32) -> u32 {
    match op {
        "const_bool" | "get_bool" | "and" | "or" | "not" | "eq" | "noteq" | "gt" | "gteq" | "lr" | "lreq" | "isdefined" => 4,
        "const_string" | "get_string" => 16384,
        "get_integer1" | "const_int1" | "toint1" | "iswizzle1" => 16,
        "get_integer2" | "const_int2" | "toint2" | "iswizzle2" | "ivector2" => 32,
        "get_integer3" | "const_int3" | "toint3" | "iswizzle3" | "ivector3" => 64,
        "get_integer4" | "const_int4" | "toint4" | "iswizzle4" | "ivector4" => 128,
        "get_float1" | "const_float1" | "tofloat" | "swizzle1" | "samplelum" | "atan2" | "dot" => 256,
        "get_float2" | "const_float2" | "tofloat2" | "swizzle2" | "vector2" | "cartesian" => 512,
        "get_float3" | "const_float3" | "tofloat3" | "swizzle3" | "vector3" => 1024,
        "get_float4" | "const_float4" | "tofloat4" | "swizzle4" | "vector4" | "samplecol" => 2048,
        "ifelse" | "sequence" => input(1),
        "while" => input(2),
        "set" => input(0),
        _ => value.map_or_else(|| input(0), value_type::type_code),
    }
}
