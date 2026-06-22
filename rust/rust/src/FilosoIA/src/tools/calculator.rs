use std::collections::HashMap;
use async_trait::async_trait;
use super::ToolParam;
use super::Tool;

pub struct Calculator;

fn eval_expr(s: &str) -> Result<f64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty expression".into());
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(n);
    }
    let (val, rest) = eval_add_sub(s)?;
    if rest.trim().is_empty() {
        Ok(val)
    } else {
        Err(format!("Unexpected trailing characters: '{}'", rest.trim()))
    }
}

fn parse_number(s: &str) -> Result<(f64, usize), String> {
    let s = s.trim_start();
    if s.is_empty() {
        return Err("Unexpected end".into());
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    if bytes[i] == b'-' || bytes[i] == b'+' {
        i += 1;
    }
    let mut has_dot = false;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || (!has_dot && bytes[i] == b'.')) {
        if bytes[i] == b'.' { has_dot = true; }
        i += 1;
    }
    if i == 0 || (i == 1 && (bytes[0] == b'-' || bytes[0] == b'+')) {
        Err("Expected number".into())
    } else {
        let num: f64 = s[..i].parse().map_err(|e| format!("Parse error: {e}"))?;
        Ok((num, s[i..].len()))
    }
}

fn eval_atom(s: &str) -> Result<(f64, &str), String> {
    let s = s.trim_start();
    if s.is_empty() {
        return Err("Unexpected end of expression".into());
    }
    if s.starts_with('(') {
        let (val, rest) = eval_add_sub(&s[1..])?;
        let rest = rest.trim_start();
        if rest.starts_with(')') {
            Ok((val, &rest[1..]))
        } else {
            Err("Missing closing parenthesis".into())
        }
    } else if s.starts_with("sqrt(") || s.starts_with("sin(") || s.starts_with("cos(") || s.starts_with("abs(") || s.starts_with("ceil(") || s.starts_with("floor(") || s.starts_with("round(") || s.starts_with("ln(") || s.starts_with("log(") || s.starts_with("exp(") {
        let func_end = s.find('(').unwrap();
        let func_name = &s[..func_end];
        let (arg, rest) = eval_add_sub(&s[func_end + 1..])?;
        let rest = rest.trim_start();
        if !rest.starts_with(')') {
            return Err("Missing closing parenthesis after function".into());
        }
        let val = match func_name {
            "sqrt" => arg.sqrt(),
            "sin" => arg.sin(),
            "cos" => arg.cos(),
            "abs" => arg.abs(),
            "ceil" => arg.ceil(),
            "floor" => arg.floor(),
            "round" => arg.round(),
            "ln" => arg.ln(),
            "log" => arg.log10(),
            "exp" => arg.exp(),
            _ => return Err(format!("Unknown function: {func_name}")),
        };
        Ok((val, &rest[1..]))
    } else {
        let (n, rest_len) = parse_number(s)?;
        let parsed_len = s.len() - rest_len;
        Ok((n, &s[parsed_len..]))
    }
}

fn eval_mul_div(s: &str) -> Result<(f64, &str), String> {
    let (mut left, mut rest) = eval_atom(s)?;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() { break; }
        let op = rest.as_bytes()[0];
        if op != b'*' && op != b'/' && op != b'%' { break; }
        let (right, new_rest) = eval_atom(&rest[1..])?;
        left = match op {
            b'*' => left * right,
            b'/' => {
                if right == 0.0 { return Err("Division by zero".into()); }
                left / right
            }
            b'%' => left % right,
            _ => unreachable!(),
        };
        rest = new_rest;
    }
    Ok((left, rest))
}

fn eval_add_sub(s: &str) -> Result<(f64, &str), String> {
    let (mut left, mut rest) = eval_mul_div(s)?;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() { break; }
        let op = rest.as_bytes()[0];
        if op != b'+' && op != b'-' { break; }
        let (right, new_rest) = eval_mul_div(&rest[1..])?;
        left = match op {
            b'+' => left + right,
            b'-' => left - right,
            _ => unreachable!(),
        };
        rest = new_rest;
    }
    Ok((left, rest))
}

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &'static str { "calculator" }
    fn description(&self) -> &'static str { "Evaluate a mathematical expression" }
    fn parameters(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "expression",
            param_type: "string",
            description: "The mathematical expression to evaluate, e.g. '2 + 2' or 'sqrt(16)'",
            required: true,
        }]
    }
    async fn call(&self, args: HashMap<String, String>) -> Result<String, String> {
        let expr = args.get("expression").ok_or("Missing 'expression' argument")?;
        match eval_expr(expr) {
            Ok(result) => Ok(result.to_string()),
            Err(e) => Err(format!("Evaluation error: {e}")),
        }
    }
}
