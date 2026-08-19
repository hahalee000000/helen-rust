//! Minimal safe arithmetic evaluator for the `calculate` tool (Task 6.3).
//! Supports + - * / % ^ ( ) and the common math functions.

/// Evaluate a simple arithmetic expression. Returns Err on any syntax error.
pub fn eval_simple(expression: &str) -> Result<String, String> {
    let tokens = tokenize(expression)?;
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.parse_expr()?;
    parser.expect_end()?;
    Ok(format_value(value))
}

fn format_value(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        num.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let v: f64 = num.parse().map_err(|_| format!("Invalid number: {num}"))?;
                tokens.push(Tok::Num(v));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Tok::Ident(ident));
            }
            '+' => {
                tokens.push(Tok::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Tok::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(Tok::Star);
                chars.next();
            }
            '/' => {
                tokens.push(Tok::Slash);
                chars.next();
            }
            '%' => {
                tokens.push(Tok::Percent);
                chars.next();
            }
            '^' => {
                tokens.push(Tok::Caret);
                chars.next();
            }
            '(' => {
                tokens.push(Tok::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Tok::RParen);
                chars.next();
            }
            ',' => {
                tokens.push(Tok::Comma);
                chars.next();
            }
            other => return Err(format!("Unexpected character: {other}")),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect_end(&self) -> Result<(), String> {
        if self.pos == self.tokens.len() {
            Ok(())
        } else {
            Err("Unexpected trailing tokens".into())
        }
    }

    fn parse_expr(&mut self) -> Result<f64, String> {
        // + / - term ((+|-|%) term)*
        let mut value = self.parse_term()?;
        loop {
            match self.peek() {
                Some(Tok::Plus) => {
                    self.next();
                    value += self.parse_term()?;
                }
                Some(Tok::Minus) => {
                    self.next();
                    value -= self.parse_term()?;
                }
                Some(Tok::Percent) => {
                    self.next();
                    let rhs = self.parse_term()?;
                    if rhs == 0.0 {
                        return Err("Modulo by zero".into());
                    }
                    value = value.rem_euclid(rhs);
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        // factor ((*|/) factor)*
        let mut value = self.parse_power()?;
        loop {
            match self.peek() {
                Some(Tok::Star) => {
                    self.next();
                    value *= self.parse_power()?;
                }
                Some(Tok::Slash) => {
                    self.next();
                    let rhs = self.parse_power()?;
                    if rhs == 0.0 {
                        return Err("Division by zero".into());
                    }
                    value /= rhs;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let base = self.parse_unary()?;
        if let Some(Tok::Caret) = self.peek() {
            self.next();
            let exp = self.parse_unary()?;
            return Ok(base.powf(exp));
        }
        Ok(base)
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.next();
                Ok(-self.parse_unary()?)
            }
            Some(Tok::Plus) => {
                self.next();
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Num(v)) => Ok(v),
            Some(Tok::LParen) => {
                let v = self.parse_expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err("Expected ')'".into()),
                }
            }
            Some(Tok::Ident(name)) => self.parse_function(name),
            _ => Err("Expected a number or '('".into()),
        }
    }

    fn parse_function(&mut self, name: String) -> Result<f64, String> {
        match self.next() {
            Some(Tok::LParen) => {}
            _ => return Err(format!("Expected '(' after '{name}'")),
        }
        let mut args = Vec::new();
        if let Some(Tok::RParen) = self.peek() {
            self.next();
        } else {
            loop {
                args.push(self.parse_expr()?);
                match self.next() {
                    Some(Tok::Comma) => continue,
                    Some(Tok::RParen) => break,
                    _ => return Err("Expected ',' or ')' in arguments".into()),
                }
            }
        }
        let v = match name.as_str() {
            "sqrt" => {
                require_args(&name, &args, 1)?;
                args[0].sqrt()
            }
            "abs" => {
                require_args(&name, &args, 1)?;
                args[0].abs()
            }
            "sin" => {
                require_args(&name, &args, 1)?;
                args[0].sin()
            }
            "cos" => {
                require_args(&name, &args, 1)?;
                args[0].cos()
            }
            "tan" => {
                require_args(&name, &args, 1)?;
                args[0].tan()
            }
            "log" => {
                require_args(&name, &args, 1)?;
                args[0].ln()
            }
            "log10" => {
                require_args(&name, &args, 1)?;
                args[0].log10()
            }
            "exp" => {
                require_args(&name, &args, 1)?;
                args[0].exp()
            }
            "floor" => {
                require_args(&name, &args, 1)?;
                args[0].floor()
            }
            "ceil" => {
                require_args(&name, &args, 1)?;
                args[0].ceil()
            }
            "round" => {
                require_args(&name, &args, 1)?;
                args[0].round()
            }
            "min" => {
                if args.is_empty() {
                    return Err("min expects at least 1 argument".into());
                }
                args.iter().copied().fold(f64::INFINITY, f64::min)
            }
            "max" => {
                if args.is_empty() {
                    return Err("max expects at least 1 argument".into());
                }
                args.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            }
            "pow" => {
                require_args(&name, &args, 2)?;
                args[0].powf(args[1])
            }
            _ => return Err(format!("Unknown function: {name}")),
        };
        Ok(v)
    }
}

fn require_args(name: &str, args: &[f64], n: usize) -> Result<(), String> {
    if args.len() == n {
        Ok(())
    } else {
        Err(format!(
            "{name} expects {n} argument(s), got {}",
            args.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arith() {
        assert_eq!(eval_simple("1 + 2 * 3").expect("eval"), "7");
        assert_eq!(eval_simple("(1 + 2) * 3").unwrap(), "9");
        assert_eq!(eval_simple("10 / 4").expect("eval"), "2.5");
        assert_eq!(eval_simple("7 % 3").expect("eval"), "1");
        assert_eq!(eval_simple("2 ^ 10").expect("eval"), "1024");
        assert_eq!(eval_simple("-5 + 3").expect("eval"), "-2");
    }

    #[test]
    fn test_functions() {
        assert_eq!(eval_simple("sqrt(16)").unwrap(), "4");
        assert_eq!(eval_simple("max(1, 5, 3)").unwrap(), "5");
        assert_eq!(eval_simple("min(1, 5, 3)").unwrap(), "1");
        assert_eq!(eval_simple("round(2.5)").unwrap(), "3");
        assert_eq!(eval_simple("abs(-3)").unwrap(), "3");
    }

    #[test]
    fn test_errors() {
        assert!(eval_simple("1/0").is_err());
        assert!(eval_simple("2 +").is_err());
        assert!(eval_simple("unknown_fn(1)").is_err());
    }
}
