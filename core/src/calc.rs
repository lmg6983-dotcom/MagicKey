//! 계산기: "1,200 × 3" · "15% * 80000" · "sqrt(2)" · "5 inch to cm"
//!   calc_input 이 계산할 만한 글인지 고르고(숫자만 · 날짜 · 전화번호 · 보통 글은 아님), 작은 파서가 계산한다.

use crate::Answer;
use fancy_regex::Regex;
use std::sync::LazyLock;

static THOUSANDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d),(?=\d{3}(\D|$))").unwrap());
static SERIAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+([-./])\d+(\1\d+)+$").unwrap());
static OP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[+\-*/^%!]|\d\s*\(|\b(sqrt|cbrt|abs|round|floor|ceil|log|log10|log2|exp|sin|cos|tan|asin|acos|atan|pi|e|mod|to|in)\b").unwrap()
});

/// 계산할 만한 글이면 계산할 식, 아니면 None
pub fn calc_input(text: &str) -> Option<String> {
    let s = text.trim();
    if s.is_empty() || s.contains('\n') || s.chars().count() > 200 {
        return None;
    }
    let s = s.strip_prefix('=').unwrap_or(s).trim_start();
    let s = s.trim_end_matches(|c: char| c == '=' || c.is_whitespace());
    let s = s.replace('×', "*").replace('÷', "/").replace('−', "-");
    let s = THOUSANDS.replace_all(&s, "$1").into_owned(); // 1,200 → 1200
    if !s.chars().any(|c| c.is_ascii_digit()) || s.chars().any(|c| ('가'..='힣').contains(&c)) {
        return None;
    }
    // 2026-10-22 · 010-1234-5678 · 1.2.3 은 계산이 아니라 날짜 · 번호
    if SERIAL.is_match(&s).unwrap_or(false) {
        return None;
    }
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(&s);
    OP.is_match(unsigned).unwrap_or(false).then_some(s) // 숫자 하나만은 계산이 아님
}

/// 쓴 글이 식이면 결과 줄
pub fn calc(text: &str) -> Option<Answer> {
    let expr = calc_input(text)?;
    let v = Parser::new(&expr)?.run()?;
    if !v.n.is_finite() {
        return None;
    }
    let plain = match v.unit {
        Some(u) => format!("{} {}", fmt_num(v.n), u.name),
        None => fmt_num(v.n),
    };
    Some(Answer { value: group_digits(&plain), plain, detail: text.trim().to_string() })
}

/// 12자리 유효 숫자, 아주 크거나 작으면 1.5e+21 꼴 (math.js format 과 같게)
fn fmt_num(n: f64) -> String {
    if n == 0.0 {
        return "0".into();
    }
    let exp = n.abs().log10().floor() as i32;
    if !(-9..21).contains(&exp) {
        let s = format!("{n:.11e}");
        let (m, e) = s.split_once('e').unwrap();
        let m = trim_zeros(m);
        let e: i32 = e.parse().unwrap();
        return format!("{m}e{}{}", if e < 0 { "-" } else { "+" }, e.abs());
    }
    let n = if exp > 11 { let p = 10f64.powi(exp - 11); (n / p).round() * p } else { n };
    trim_zeros(&format!("{:.*}", (11 - exp).max(0) as usize, n)).to_string()
}

fn trim_zeros(s: &str) -> &str {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.') } else { s }
}

/// 정수 부분에 천 단위 쉼표
fn group_digits(s: &str) -> String {
    if s.contains('e') {
        return s.into();
    }
    let (sign, rest) = s.strip_prefix('-').map_or(("", s), |r| ("-", r));
    let int_len = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let (int, tail) = rest.split_at(int_len);
    let mut out = String::from(sign);
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out + tail
}

// ---------- 단위 (길이 · 무게 · 부피만) ----------
#[derive(Debug, Clone, Copy, PartialEq)]
struct Unit {
    name: &'static str,
    dim: u8,
    factor: f64, // 기본 단위(m · g · l)로
}

const UNITS: &[(&[&str], Unit)] = &[
    (&["mm"], Unit { name: "mm", dim: 0, factor: 0.001 }),
    (&["cm"], Unit { name: "cm", dim: 0, factor: 0.01 }),
    (&["m"], Unit { name: "m", dim: 0, factor: 1.0 }),
    (&["km"], Unit { name: "km", dim: 0, factor: 1000.0 }),
    (&["inch", "inches"], Unit { name: "inch", dim: 0, factor: 0.0254 }),
    (&["ft", "foot", "feet"], Unit { name: "ft", dim: 0, factor: 0.3048 }),
    (&["yd", "yard", "yards"], Unit { name: "yd", dim: 0, factor: 0.9144 }),
    (&["mi", "mile", "miles"], Unit { name: "mi", dim: 0, factor: 1609.344 }),
    (&["mg"], Unit { name: "mg", dim: 1, factor: 0.001 }),
    (&["g"], Unit { name: "g", dim: 1, factor: 1.0 }),
    (&["kg"], Unit { name: "kg", dim: 1, factor: 1000.0 }),
    (&["lb", "lbs"], Unit { name: "lb", dim: 1, factor: 453.59237 }),
    (&["oz"], Unit { name: "oz", dim: 1, factor: 28.349523125 }),
    (&["ml"], Unit { name: "ml", dim: 2, factor: 0.001 }),
    (&["l"], Unit { name: "l", dim: 2, factor: 1.0 }),
];

fn unit(word: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|(names, _)| names.contains(&word)).map(|(_, u)| u)
}

#[derive(Debug, Clone, Copy)]
struct Val {
    n: f64,
    unit: Option<&'static Unit>,
}

impl Val {
    const fn num(n: f64) -> Self {
        Val { n, unit: None }
    }
    fn to(self, u: &'static Unit) -> Option<Val> {
        let from = self.unit?;
        (from.dim == u.dim).then(|| Val { n: self.n * from.factor / u.factor, unit: Some(u) })
    }
}

// ---------- 파서 ----------
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Word(String),
    Op(char),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(s: &str) -> Option<Self> {
        let cs: Vec<char> = s.chars().collect();
        let mut toks = Vec::new();
        let mut i = 0;
        while i < cs.len() {
            let c = cs[i];
            if c.is_whitespace() {
                i += 1;
            } else if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(char::is_ascii_digit)) {
                let start = i;
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                    i += 1;
                }
                // 1e400 · 2.5e-3
                if i < cs.len() && (cs[i] == 'e' || cs[i] == 'E') {
                    let mut j = i + 1;
                    if j < cs.len() && (cs[j] == '+' || cs[j] == '-') {
                        j += 1;
                    }
                    if j < cs.len() && cs[j].is_ascii_digit() {
                        i = j;
                        while i < cs.len() && cs[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
                toks.push(Tok::Num(cs[start..i].iter().collect::<String>().parse().ok()?));
            } else if c.is_alphabetic() {
                let start = i;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Word(cs[start..i].iter().collect::<String>().to_lowercase()));
            } else if "+-*/^%!(),".contains(c) {
                toks.push(Tok::Op(c));
                i += 1;
            } else {
                return None;
            }
        }
        Some(Parser { toks, pos: 0 })
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn eat(&mut self, c: char) -> bool {
        let hit = self.peek() == Some(&Tok::Op(c));
        if hit {
            self.pos += 1;
        }
        hit
    }
    fn eat_word(&mut self, w: &str) -> bool {
        let hit = matches!(self.peek(), Some(Tok::Word(x)) if x == w);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn run(mut self) -> Option<Val> {
        let mut v = self.add()?;
        if self.eat_word("to") || self.eat_word("in") {
            let Some(Tok::Word(w)) = self.peek().cloned() else { return None };
            self.pos += 1;
            v = v.to(unit(&w)?)?;
        }
        (self.pos == self.toks.len()).then_some(v)
    }

    fn add(&mut self) -> Option<Val> {
        let mut a = self.mul()?;
        loop {
            let sign = if self.eat('+') { 1.0 } else if self.eat('-') { -1.0 } else { return Some(a) };
            let b = self.mul()?;
            a = match (a.unit, b.unit) {
                (None, None) => Val::num(a.n + sign * b.n),
                (Some(u), Some(_)) => Val { n: a.n + sign * b.to(u)?.n, unit: Some(u) },
                _ => return None,
            };
        }
    }

    // 다음 낱말이 곱하기를 생략한 값의 시작인가 (2pi · 2(3+4)) — to · in · mod 는 아님
    fn implicit(&self) -> bool {
        match self.peek() {
            Some(Tok::Op('(')) | Some(Tok::Num(_)) => true,
            Some(Tok::Word(w)) => !matches!(w.as_str(), "to" | "in" | "mod"),
            _ => false,
        }
    }

    fn mul(&mut self) -> Option<Val> {
        let mut a = self.unary()?;
        loop {
            let op = if self.eat('*') { '*' } else if self.eat('/') { '/' } else if self.eat('%') || self.eat_word("mod") { '%' } else if self.implicit() { '*' } else { return Some(a) };
            let b = self.unary()?;
            if a.unit.is_some() && b.unit.is_some() {
                return None; // 단위끼리 곱하기는 하지 않음
            }
            let n = match op {
                '*' => a.n * b.n,
                '/' if b.unit.is_some() => return None,
                '/' => a.n / b.n,
                _ => a.n.rem_euclid(b.n),
            };
            a = Val { n, unit: a.unit.or(b.unit) };
        }
    }

    fn unary(&mut self) -> Option<Val> {
        if self.eat('-') {
            let v = self.unary()?;
            return Some(Val { n: -v.n, ..v });
        }
        if self.eat('+') {
            return self.unary();
        }
        self.pow()
    }

    fn pow(&mut self) -> Option<Val> {
        let base = self.postfix()?;
        if self.eat('^') {
            let e = self.unary()?;
            if base.unit.is_some() || e.unit.is_some() {
                return None;
            }
            return Some(Val::num(base.n.powf(e.n)));
        }
        Some(base)
    }

    // 15% (퍼센트: 뒤에 값이 오지 않을 때만, 오면 나머지) · 5! · 5 cm
    fn postfix(&mut self) -> Option<Val> {
        let mut v = self.primary()?;
        loop {
            if self.peek() == Some(&Tok::Op('%')) && !matches!(self.toks.get(self.pos + 1), Some(Tok::Num(_)) | Some(Tok::Op('(')) | Some(Tok::Word(_))) {
                self.pos += 1;
                v.n /= 100.0;
            } else if self.eat('!') {
                if v.n < 0.0 || v.n.fract() != 0.0 || v.n > 170.0 {
                    return None;
                }
                v.n = (1..=v.n as u32).map(f64::from).product();
            } else if let Some(Tok::Word(w)) = self.peek() {
                let Some(u) = unit(w).filter(|_| v.unit.is_none()) else { return Some(v) };
                self.pos += 1;
                v.unit = Some(u);
            } else {
                return Some(v);
            }
        }
    }

    fn primary(&mut self) -> Option<Val> {
        match self.peek()?.clone() {
            Tok::Num(n) => {
                self.pos += 1;
                Some(Val::num(n))
            }
            Tok::Op('(') => {
                self.pos += 1;
                let v = self.add()?;
                self.eat(')').then_some(v)
            }
            Tok::Word(w) => {
                self.pos += 1;
                match w.as_str() {
                    "pi" => return Some(Val::num(std::f64::consts::PI)),
                    "e" => return Some(Val::num(std::f64::consts::E)),
                    _ => {}
                }
                if !self.eat('(') {
                    return None;
                }
                let mut args = vec![self.add()?];
                while self.eat(',') {
                    args.push(self.add()?);
                }
                if !self.eat(')') || args.iter().any(|a| a.unit.is_some()) {
                    return None;
                }
                let x = args[0].n;
                let n = match (w.as_str(), args.len()) {
                    ("sqrt", 1) => x.sqrt(),
                    ("cbrt", 1) => x.cbrt(),
                    ("abs", 1) => x.abs(),
                    ("round", 1) => x.round(),
                    ("floor", 1) => x.floor(),
                    ("ceil", 1) => x.ceil(),
                    ("log", 1) => x.ln(),
                    ("log", 2) => x.log(args[1].n),
                    ("log10", 1) => x.log10(),
                    ("log2", 1) => x.log2(),
                    ("exp", 1) => x.exp(),
                    ("sin", 1) => x.sin(),
                    ("cos", 1) => x.cos(),
                    ("tan", 1) => x.tan(),
                    ("asin", 1) => x.asin(),
                    ("acos", 1) => x.acos(),
                    ("atan", 1) => x.atan(),
                    _ => return None,
                };
                Some(Val::num(n))
            }
            Tok::Op(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(t: &str) -> Option<(String, String)> {
        calc(t).map(|a| (a.value, a.plain))
    }

    #[test]
    fn expressions() {
        assert_eq!(c("1,200 × 3"), Some(("3,600".into(), "3600".into())));
        assert_eq!(c("= 10 ÷ 4"), Some(("2.5".into(), "2.5".into())));
        assert_eq!(c("15% * 80000").unwrap().1, "12000");
        assert_eq!(c("0.1+0.2").unwrap().1, "0.3");
        assert_eq!(c("5 inch to cm").unwrap().1, "12.7 cm");
        assert_eq!(c("sqrt(16)").unwrap().1, "4");
        assert_eq!(c("1234567 * 10").unwrap().0, "12,345,670");
        assert_eq!(c("2^10").unwrap().1, "1024");
        assert_eq!(c("-2^2").unwrap().1, "-4");
        assert_eq!(c("10 % 3").unwrap().1, "1");
        assert_eq!(c("5!").unwrap().1, "120");
        assert_eq!(c("2(3+4)").unwrap().1, "14");
        assert_eq!(c("1/3").unwrap().1, "0.333333333333");
        assert_eq!(c("1e21*1.5").unwrap().1, "1.5e+21");
        assert_eq!(c("3 kg + 500 g").unwrap().1, "3.5 kg");
    }

    #[test]
    fn not_expressions() {
        for t in ["42", "-5", "2026-10-22", "010-1234-5678", "10/22/2026", "1.2.3", "회의 3-4시", "보고서 2부", "", "1+1\n2"] {
            assert_eq!(calc_input(t), None, "{t}");
        }
        assert_eq!(calc("abc+1"), None, "모르는 낱말");
        assert_eq!(calc("1e400*10"), None, "무한대");
        assert_eq!(calc("5 kg to cm"), None, "다른 종류 단위");
    }
}
