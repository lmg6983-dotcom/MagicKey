//! 날짜 계산 — 인터넷 없이, 쓴 글 전체가 날짜 계산일 때만 답한다 ("3일 뒤 출장 준비" 같은 메모 글은 계산하지 않게)
//!
//!   오늘에서:   3주 뒤 · 100일 후 · 2달 전 · 1년 반 뒤 · 열흘 뒤 · 100일째 · D-100 · D+30 · 영업일 10일 뒤
//!   날짜에서:   2026-03-01 + 100일 · 3월 1일부터 100일째 · 12/25 - 2주 · 크리스마스로부터 30일 뒤
//!   남은 날:    12월 25일까지 · 12/25까지 며칠 · 디데이 2027-01-01 · 금요일까지
//!   지난 날:    2026-03-01부터 · 3월 1일부터 오늘까지 며칠
//!   사이:       1월 1일부터 6월 30일까지 · 2026.1.1 ~ 2026.6.30 (양 끝 포함 · 주 · 달 · 영업일도)
//!   요일:       12월 25일 무슨 요일 · 다음 주 금요일 · 다음 달 말일 · 2026-12-25 (날짜만 써도)
//!   시각:       3시간 뒤 · 90분 후 · 오후 3시 + 2시간 · 9:15부터 18:30까지 · 14:30 - 9:15
//!   나이 · 오늘: 만 나이 1990-05-03 · 오늘 몇 주차 · 올해 남은 날
//!
//!   해가 없는 날짜는 "까지"면 다가오는 날, "부터"면 지나간 날, 그 밖은 올해.
//!   달을 더해 없는 날이 되면 그 달 말일, 영업일은 토 · 일과 양력 공휴일만 뺀다.

use crate::Answer;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use fancy_regex::{Captures, Regex};
use std::sync::LazyLock;

type D = NaiveDate;

const WEEKDAY: [char; 7] = ['일', '월', '화', '수', '목', '금', '토'];

// ---------- 날 ----------
fn mk(y: i32, m0: i32, d: u32) -> Option<D> {
    D::from_ymd_opt(y, (m0 + 1) as u32, d)
}
fn last_day(y: i32, m0: i32) -> u32 {
    let (ny, nm) = if m0 == 11 { (y + 1, 0) } else { (y, m0 + 1) };
    (mk(ny, nm, 1).unwrap() - Duration::days(1)).day()
}
fn valid(y: i32, m0: i32, d: u32) -> bool {
    (0..=11).contains(&m0) && d >= 1 && d <= last_day(y, m0)
}
fn add_d(d: D, n: i64) -> D {
    d + Duration::days(n)
}
fn diff(a: D, b: D) -> i64 {
    (b - a).num_days()
}
fn wd(d: D) -> i64 {
    d.weekday().num_days_from_sunday() as i64
}
fn m0(d: D) -> i32 {
    d.month0() as i32
}

/// 달 더하기 — 없는 날이면 그 달 말일 (clamped: 말일로 바꿨는가)
fn add_months(d: D, n: i32) -> (D, bool) {
    let m = m0(d) + n;
    let ty = d.year() + m.div_euclid(12);
    let tm = m.rem_euclid(12);
    let day = d.day().min(last_day(ty, tm));
    (mk(ty, tm, day).unwrap(), day != d.day())
}

// ---------- 공휴일 (양력만 — 설 · 추석 · 부처님오신날 · 대체공휴일은 해마다 달라 빼지 않는다) ----------
const SOLAR_HOLIDAYS: [(u32, u32); 8] = [(1, 1), (3, 1), (5, 5), (6, 6), (8, 15), (10, 3), (10, 9), (12, 25)];
fn is_off(d: D) -> bool {
    wd(d) == 0 || wd(d) == 6 || SOLAR_HOLIDAYS.contains(&(d.month(), d.day()))
}
fn add_business_days(d: D, n: i64) -> D {
    let mut cur = d;
    let mut left = n.abs();
    let step = if n < 0 { -1 } else { 1 };
    while left > 0 {
        cur = add_d(cur, step);
        if !is_off(cur) {
            left -= 1;
        }
    }
    cur
}
fn business_days_between(a: D, b: D) -> i64 {
    let mut n = 0;
    let mut cur = add_d(a, 1);
    while cur <= b {
        if !is_off(cur) {
            n += 1;
        }
        cur = add_d(cur, 1);
    }
    n
}

// 이름 있는 날 (양력, 해마다 같은 날)
const NAMED_DAYS: &[(&str, u32, u32)] = &[
    ("크리스마스", 12, 25), ("성탄절", 12, 25), ("새해", 1, 1), ("신정", 1, 1), ("삼일절", 3, 1), ("어린이날", 5, 5), ("어버이날", 5, 8),
    ("스승의날", 5, 15), ("현충일", 6, 6), ("제헌절", 7, 17), ("광복절", 8, 15), ("개천절", 10, 3), ("한글날", 10, 9),
    ("발렌타인데이", 2, 14), ("밸런타인데이", 2, 14), ("화이트데이", 3, 14), ("빼빼로데이", 11, 11), ("할로윈", 10, 31), ("핼러윈", 10, 31),
];
fn named(s: &str) -> Option<(u32, u32)> {
    NAMED_DAYS.iter().find(|(n, ..)| *n == s).map(|&(_, m, d)| (m, d))
}

// ---------- 날짜 말 ----------
const NATIVE: &[(&str, f64)] = &[
    ("한", 1.0), ("두", 2.0), ("세", 3.0), ("석", 3.0), ("네", 4.0), ("넉", 4.0), ("다섯", 5.0), ("여섯", 6.0), ("일곱", 7.0),
    ("여덟", 8.0), ("아홉", 9.0), ("열", 10.0), ("열한", 11.0), ("열두", 12.0),
];
const NATIVE_DAYS: &[(&str, f64)] = &[
    ("하루", 1.0), ("이틀", 2.0), ("사흘", 3.0), ("나흘", 4.0), ("닷새", 5.0), ("엿새", 6.0), ("이레", 7.0), ("여드레", 8.0), ("아흐레", 9.0), ("열흘", 10.0), ("보름", 15.0),
];
const RELATIVE_DAY: &[(&str, i64)] = &[
    ("그끄저께", -3), ("그저께", -2), ("그제", -2), ("어제", -1), ("오늘", 0), ("지금", 0), ("내일", 1), ("모레", 2), ("글피", 3), ("그글피", 4),
];
const WEEK_OFFSET: &[(&str, i64)] = &[("지지난", -2), ("지난", -1), ("저번", -1), ("이번", 0), ("금", 0), ("다음", 1), ("담", 1), ("다다음", 2)];
const YEAR_OFFSET: &[(&str, i32)] = &[("재작년", -2), ("작년", -1), ("지난해", -1), ("올해", 0), ("금년", 0), ("내년", 1), ("다음해", 1), ("내후년", 2)];

fn lookup<T: Copy>(list: &[(&str, T)], key: &str) -> Option<T> {
    list.iter().find(|(k, _)| *k == key).map(|&(_, v)| v)
}
fn keys<T>(list: &[(&str, T)]) -> String {
    list.iter().map(|(k, _)| *k).collect::<Vec<_>>().join("|")
}

#[derive(Clone, Copy, PartialEq)]
pub enum Prefer {
    Near,
    Future,
    Past,
}

#[derive(Debug, Clone, Copy)]
pub struct Day {
    pub date: D,
    pub explicit_year: bool,
    pub relative: bool,
}
fn day(date: D) -> Day {
    Day { date, explicit_year: false, relative: false }
}
fn day_y(date: D) -> Day {
    Day { date, explicit_year: true, relative: false }
}

/// 해 없이 쓴 달 · 날의 해
fn pick_year(m: i32, d: u32, today: D, prefer: Prefer) -> Option<D> {
    let y = today.year();
    if !valid(y, m, d) && !(m == 1 && d == 29) {
        return None;
    }
    let try_year = |yy: i32| valid(yy, m, d).then(|| mk(yy, m, d).unwrap());
    match prefer {
        Prefer::Future => (y..=y + 8).filter_map(try_year).find(|c| *c >= today),
        Prefer::Past => (y - 8..=y).rev().filter_map(try_year).find(|c| *c <= today),
        Prefer::Near => try_year(y).or_else(|| try_year(y + 1)),
    }
}

fn full_year(yy: &str) -> i32 {
    let n: i32 = yy.parse().unwrap_or(0);
    if yy.len() == 2 { 2000 + n } else { n }
}

fn re(p: &str) -> Regex {
    Regex::new(p).unwrap()
}
fn caps<'a>(r: &Regex, s: &'a str) -> Option<Captures<'a>> {
    r.captures(s).ok().flatten()
}
fn g<'a>(c: &Captures<'a>, i: usize) -> Option<&'a str> {
    c.get(i).map(|m| m.as_str())
}
fn num<T: std::str::FromStr + Default>(c: &Captures, i: usize) -> T {
    g(c, i).and_then(|s| s.parse().ok()).unwrap_or_default()
}
fn is(r: &Regex, s: &str) -> bool {
    r.is_match(s).unwrap_or(false)
}
fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

static P_ISO: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{4}|\d{2})\s*([-./])\s*(\d{1,2})\s*\2\s*(\d{1,2})\.?$"));
static P_YMD: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(r"^(?:(\d{{2,4}})\s*년|({}))\s*(\d{{1,2}})\s*월(?:\s*(\d{{1,2}})\s*일|\s*(말일?|말))?$", keys(YEAR_OFFSET)))
});
static P_MD: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{1,2})\s*월\s*(?:(\d{1,2})\s*일|(말일?|말))$"));
static P_SLASH: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{1,2})\s*([-./])\s*(\d{1,2})$"));
static P_NAMED: LazyLock<Regex> = LazyLock::new(|| {
    let mut names: Vec<&str> = NAMED_DAYS.iter().map(|(n, ..)| *n).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    re(&format!(r"^(?:({})\s*)?({})$", keys(YEAR_OFFSET), names.join("|")))
});
static P_MONTH: LazyLock<Regex> = LazyLock::new(|| re(r"^(이번|이|금|다음|담|다다음|지난|저번)\s*달\s*(?:(\d{1,2})\s*일|(말일?|말|초))$"));
static P_DAY: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{1,2})\s*일$"));
static P_WEEKS_WD: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{1,2}|한|두|세|네)\s*주\s*(?:뒤|후)\s*([일월화수목금토])(?:요일)?$"));
static P_WD: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:(지지난|지난|저번|이번|금|다음|담|다다음)\s*주\s*)?([일월화수목금토])(?:요일)?$"));

fn weekday_index(s: &str) -> i64 {
    WEEKDAY.iter().position(|c| s.starts_with(*c)).unwrap_or(0) as i64
}
fn monday_of(t: D) -> D {
    add_d(t, -((wd(t) + 6) % 7))
}

/// 날짜 말 하나 → 날 (앞뒤 띄어쓰기는 정리해서 읽음)
pub fn parse_day(raw: &str, now: NaiveDateTime, prefer: Prefer) -> Option<Day> {
    let s = squash(raw);
    if s.is_empty() {
        return None;
    }
    let t = now.date();
    let nm = m0(t);
    if let Some(off) = lookup(RELATIVE_DAY, &s) {
        return Some(Day { date: add_d(t, off), explicit_year: false, relative: true });
    }
    // 2026-12-25 · 2026.12.25 · 2026/12/25 · 26.12.25
    if let Some(c) = caps(&P_ISO, &s) {
        let (y, mo, d) = (full_year(g(&c, 1)?), num::<i32>(&c, 3) - 1, num::<u32>(&c, 4));
        return valid(y, mo, d).then(|| day_y(mk(y, mo, d).unwrap()));
    }
    // 2026년 12월 25일 · 내년 3월 1일 · 2027년 1월 (1일) · 2026년 2월 말일
    if let Some(c) = caps(&P_YMD, &s) {
        let y = match g(&c, 1) {
            Some(yy) => full_year(yy),
            None => t.year() + lookup(YEAR_OFFSET, g(&c, 2)?)?,
        };
        let mo = num::<i32>(&c, 3) - 1;
        if !(0..=11).contains(&mo) {
            return None;
        }
        let d = if g(&c, 5).is_some() { last_day(y, mo) } else if g(&c, 4).is_some() { num(&c, 4) } else { 1 };
        return valid(y, mo, d).then(|| day_y(mk(y, mo, d).unwrap()));
    }
    // 12월 25일 · 12월 말일
    if let Some(c) = caps(&P_MD, &s) {
        let mo = num::<i32>(&c, 1) - 1;
        if !(0..=11).contains(&mo) {
            return None;
        }
        if g(&c, 3).is_some() {
            let y = pick_year(mo, 1, t, prefer).map_or(t.year(), |d| d.year());
            return Some(day(mk(y, mo, last_day(y, mo))?));
        }
        return pick_year(mo, num(&c, 2), t, prefer).map(day);
    }
    // 12/25 · 12.25 (해 없이)
    if let Some(c) = caps(&P_SLASH, &s) {
        return pick_year(num::<i32>(&c, 1) - 1, num(&c, 3), t, prefer).map(day);
    }
    // 크리스마스 · 내년 어린이날
    if let Some(c) = caps(&P_NAMED, &s) {
        let (mo, d) = named(g(&c, 2)?)?;
        return match g(&c, 1) {
            Some(yw) => Some(day_y(mk(t.year() + lookup(YEAR_OFFSET, yw)?, mo as i32 - 1, d)?)),
            None => pick_year(mo as i32 - 1, d, t, prefer).map(day),
        };
    }
    // 이번 달 말일 · 다음 달 15일 · 지난달 1일
    if let Some(c) = caps(&P_MONTH, &s) {
        let off = match g(&c, 1)? {
            "이번" | "이" | "금" => 0,
            "다음" | "담" => 1,
            "다다음" => 2,
            _ => -1,
        };
        let total = t.year() * 12 + nm + off;
        let (y, mo) = (total.div_euclid(12), total.rem_euclid(12));
        let d = match g(&c, 3) {
            Some("초") => 1,
            Some(_) => last_day(y, mo),
            None => num(&c, 2),
        };
        return valid(y, mo, d).then(|| day(mk(y, mo, d).unwrap()));
    }
    // 25일 (이번 달)
    if let Some(c) = caps(&P_DAY, &s) {
        let d = num(&c, 1);
        return valid(t.year(), nm, d).then(|| day(mk(t.year(), nm, d).unwrap()));
    }
    // 2주 뒤 금요일
    if let Some(c) = caps(&P_WEEKS_WD, &s) {
        let w = g(&c, 1)?;
        let n = w.parse::<i64>().ok().or_else(|| lookup(NATIVE, w).map(|v| v as i64))?;
        return Some(day(add_d(monday_of(t), n * 7 + (weekday_index(g(&c, 2)?) + 6) % 7)));
    }
    // 다음 주 금요일 · 지난주 금 · 금요일 (다가오는 · 지나간)
    if let Some(c) = caps(&P_WD, &s) {
        let w = weekday_index(g(&c, 2)?);
        if let Some(off) = g(&c, 1) {
            return Some(day(add_d(monday_of(t), lookup(WEEK_OFFSET, off)? * 7 + (w + 6) % 7)));
        }
        return Some(day(if prefer == Prefer::Past { add_d(t, -((wd(t) - w + 7) % 7)) } else { add_d(t, (w - wd(t) + 7) % 7) }));
    }
    None
}

// ---------- 기간 말 ----------
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Dur {
    pub y: f64,
    pub mo: f64,
    pub w: f64,
    pub d: f64,
    pub h: f64,
    pub min: f64,
    pub s: f64,
}
impl Dur {
    fn has_clock(&self) -> bool {
        self.h != 0.0 || self.min != 0.0 || self.s != 0.0
    }
    fn has_date(&self) -> bool {
        self.y != 0.0 || self.mo != 0.0 || self.w != 0.0 || self.d != 0.0
    }
    fn big(&self) -> bool {
        self.y != 0.0 || self.mo != 0.0 || self.w != 0.0
    }
}

static PART: LazyLock<Regex> = LazyLock::new(|| {
    let mut nat: Vec<&str> = NATIVE.iter().map(|(k, _)| *k).collect();
    nat.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    re(&format!(
        r"^(\d+(?:\.\d+)?|{})\s*(개월|주일|시간|years?|months?|weeks?|days?|hours?|minutes?|년|해|달|주|일|날|분|초)(\s*반)?",
        nat.join("|")
    ))
});

/// "1년 2개월" · "3주" · "열흘" · "1시간 반" · "반년" → 기간
pub fn parse_duration(raw: &str) -> Option<Dur> {
    let s = squash(raw);
    if s.is_empty() {
        return None;
    }
    let mut out = Dur::default();
    if let Some(d) = lookup(NATIVE_DAYS, &s) {
        return Some(Dur { d, ..out });
    }
    match s.as_str() {
        "반년" => return Some(Dur { mo: 6.0, ..out }),
        "일주일" | "한 주" | "한주" => return Some(Dur { w: 1.0, ..out }),
        "이주일" => return Some(Dur { w: 2.0, ..out }),
        "반나절" => return Some(Dur { h: 6.0, ..out }),
        "한나절" => return Some(Dur { h: 12.0, ..out }),
        _ => {}
    }
    let s = s.to_lowercase();
    let mut pos = 0;
    let mut any = false;
    while pos < s.len() {
        if s[pos..].starts_with(' ') {
            pos += 1;
            continue;
        }
        let c = caps(&PART, &s[pos..])?;
        let w = g(&c, 1)?;
        let n = w.parse::<f64>().ok().or_else(|| lookup(NATIVE, w))?;
        let half = g(&c, 3).is_some();
        match g(&c, 2)? {
            "년" | "해" | "year" | "years" => {
                out.y += n;
                if half { out.mo += 6.0 }
            }
            "개월" | "달" | "month" | "months" => {
                out.mo += n;
                if half { out.d += 15.0 }
            }
            "주" | "주일" | "week" | "weeks" => {
                out.w += n;
                if half { out.d += 3.5 }
            }
            "일" | "날" | "day" | "days" => {
                out.d += n;
                if half { out.h += 12.0 }
            }
            "시간" | "hour" | "hours" => {
                out.h += n;
                if half { out.min += 30.0 }
            }
            "분" | "minute" | "minutes" => {
                out.min += n;
                if half { out.s += 30.0 }
            }
            _ => out.s += n, // 초
        }
        any = true;
        pos += c.get(0)?.end();
    }
    // 1만 년을 넘는 기간은 계산하지 않는다 (날짜 범위를 넘으면 chrono 가 panic)
    let days = out.y * 366.0 + out.mo * 31.0 + out.w * 7.0 + out.d + out.h / 24.0 + out.min / 1440.0 + out.s / 86_400.0;
    if !any || !days.is_finite() || days > 3_660_000.0 {
        return None;
    }
    // 소수: 1.5년 · 2.5시간 → 아래 단위로
    let cascade = |hi: &mut f64, lo: &mut f64, k: f64| {
        let f = hi.fract();
        if f != 0.0 {
            *hi = hi.trunc();
            *lo += (f * k * 1000.0).round() / 1000.0;
        }
    };
    cascade(&mut out.y, &mut out.mo, 12.0);
    cascade(&mut out.mo, &mut out.d, 30.0);
    cascade(&mut out.w, &mut out.d, 7.0);
    cascade(&mut out.d, &mut out.h, 24.0);
    cascade(&mut out.h, &mut out.min, 60.0);
    cascade(&mut out.min, &mut out.s, 60.0);
    Some(out)
}

fn duration_text(dur: &Dur) -> String {
    let mut parts = Vec::new();
    for (v, u) in [(dur.y, "년"), (dur.mo, "개월"), (dur.w, "주")] {
        if v != 0.0 {
            parts.push(format!("{v}{u}"));
        }
    }
    if dur.d != 0.0 {
        parts.push(if dur.d.fract() == 0.0 { format!("{}일", dur.d) } else { format!("{:.1}일", dur.d) });
    }
    for (v, u) in [(dur.h, "시간"), (dur.min, "분"), (dur.s, "초")] {
        if v != 0.0 {
            parts.push(format!("{v}{u}"));
        }
    }
    parts.join(" ")
}

/// 날짜에 기간 더하기 — 해 · 달은 달력으로(말일 맞춤), 주 · 날은 날 수로
fn shift_date(d: D, dur: &Dur, sign: f64) -> (D, bool) {
    let (date, clamped) = add_months(d, (sign * (dur.y * 12.0 + dur.mo)) as i32);
    (add_d(date, (sign * (dur.w * 7.0 + dur.d)).trunc() as i64), clamped)
}

// ---------- 시각 말 ----------
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub h: i64,
    pub min: i64,
    pub ampm: bool,
}

static C_COLON: LazyLock<Regex> = LazyLock::new(|| re(r"^(오전|오후|새벽|아침|낮|저녁|밤)?\s*(\d{1,2}):(\d{2})$"));
static C_SI: LazyLock<Regex> = LazyLock::new(|| re(r"^(오전|오후|새벽|아침|낮|저녁|밤)?\s*(\d{1,2})\s*시(?:\s*(?:(\d{1,2})\s*분|(반)))?$"));

/// "오후 3시 30분" · "15:30" · "3시 반" · "정오" · "자정"
pub fn parse_clock(raw: &str) -> Option<Clock> {
    let s = squash(raw);
    match s.as_str() {
        "정오" => return Some(Clock { h: 12, min: 0, ampm: true }),
        "자정" => return Some(Clock { h: 0, min: 0, ampm: true }),
        _ => {}
    }
    if let Some(c) = caps(&C_COLON, &s) {
        return with_period(num(&c, 2), num(&c, 3), g(&c, 1));
    }
    let c = caps(&C_SI, &s)?;
    with_period(num(&c, 2), if g(&c, 4).is_some() { 30 } else { num(&c, 3) }, g(&c, 1))
}
fn with_period(h: i64, min: i64, period: Option<&str>) -> Option<Clock> {
    if h > 24 || min > 59 {
        return None;
    }
    let Some(p) = period else { return Some(Clock { h: h % 24, min, ampm: h > 12 || h == 0 }) };
    let pm = matches!(p, "오후" | "저녁" | "밤") || (p == "낮" && h < 6);
    if h > 12 {
        return None;
    }
    Some(Clock { h: if pm { h % 12 + 12 } else { h % 12 }, min, ampm: true })
}

// ---------- 보여 주기 ----------
fn day_text(d: D) -> String {
    format!("{}년 {}월 {}일 ({})", d.year(), d.month(), d.day(), WEEKDAY[wd(d) as usize])
}
fn short_day(d: D, t: D) -> String {
    if d.year() == t.year() { format!("{}월 {}일 ({})", d.month(), d.day(), WEEKDAY[wd(d) as usize]) } else { day_text(d) }
}
fn iso_day(d: D) -> String {
    format!("{}-{:02}-{:02}", d.year(), d.month(), d.day()) // chrono 의 %Y 는 다섯 자리 해에 + 를 붙인다
}
fn clock_text(h: i64, min: i64) -> String {
    format!("{} {}:{min:02}", if h < 12 { "오전" } else { "오후" }, if h % 12 == 0 { 12 } else { h % 12 })
}
fn rel_word(n: i64) -> Option<&'static str> {
    Some(match n {
        -2 => "그저께",
        -1 => "어제",
        0 => "오늘",
        1 => "내일",
        2 => "모레",
        _ => return None,
    })
}
fn rel(n: i64) -> String {
    rel_word(n).map_or_else(|| if n > 0 { format!("{n}일 뒤") } else { format!("{}일 전", -n) }, String::from)
}

/// 날 수 → "9주 1일" (7일 이상일 때만)
fn weeks_text(n: i64) -> String {
    let a = n.abs();
    if a < 7 {
        return String::new();
    }
    if a % 7 == 0 { format!("{}주", a / 7) } else { format!("{}주 {}일", a / 7, a % 7) }
}

/// 두 날 사이 해 · 달 · 날 ("1년 2개월 3일")
fn ymd_between(a: D, b: D) -> String {
    let (from, to) = if a <= b { (a, b) } else { (b, a) };
    let mut months = (to.year() - from.year()) * 12 + m0(to) - m0(from);
    if add_months(from, months).0 > to {
        months -= 1;
    }
    let days = diff(add_months(from, months).0, to);
    let (y, mo) = (months / 12, months % 12);
    let mut parts = Vec::new();
    if y != 0 {
        parts.push(format!("{y}년"));
    }
    if mo != 0 {
        parts.push(format!("{mo}개월"));
    }
    if days != 0 || (y == 0 && mo == 0) {
        parts.push(format!("{days}일"));
    }
    parts.join(" ")
}

fn join(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(", ")
}
fn ans(value: impl Into<String>, plain: impl Into<String>, detail: impl Into<String>) -> Answer {
    Answer { value: value.into(), plain: plain.into(), detail: detail.into() }
}

fn date_result(date: D, t: D, detail: &str) -> Answer {
    let r = rel(diff(t, date));
    let extra = if detail.contains(&r) { String::new() } else { r };
    ans(day_text(date), iso_day(date), join(&[detail, &extra]))
}

fn until_result(date: D, t: D, label: &str) -> Answer {
    let n = diff(t, date);
    let when = short_day(date, t);
    let lead = if label.is_empty() { String::new() } else { format!("{label} ") };
    if n == 0 {
        return ans("오늘이에요", "0", format!("{}, D-day", day_text(date)));
    }
    if n > 0 {
        return ans(format!("{n}일 남음"), n.to_string(), join(&[&format!("{lead}{when}까지"), &weeks_text(n), &format!("D-{n}")]));
    }
    ans(format!("{}일 지남", -n), (-n).to_string(), join(&[&format!("{lead}{when}"), &weeks_text(n), &format!("D+{}", -n)]))
}

fn range_result(a: D, b: D, t: D) -> Answer {
    let (from, to) = if a <= b { (a, b) } else { (b, a) };
    let n = diff(from, to);
    let mut parts = vec![format!("{} ~ {}", short_day(from, t), short_day(to, t)), weeks_text(n)];
    if n >= 28 {
        parts.push(ymd_between(from, to));
    }
    parts.push(format!("양 끝 포함 {}일", n + 1));
    parts.push(format!("영업일 {}일", business_days_between(from, to) + if is_off(from) { 0 } else { 1 }));
    ans(format!("{n}일"), n.to_string(), join(&parts.iter().map(String::as_str).collect::<Vec<_>>()))
}

// ---------- 읽기 ----------
// 끝에 붙는 물음 말 (떼고 읽는다): "12월 25일까지 며칠 남았어?" · "무슨 요일이야"
static TAIL: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\s*(?:[?？!.]+|(?<![남되])(?:은|는)|이야|야|예요|이에요|인가요|일까요?|이지|이죠|죠|이냐|냐|니|입니까|인지|알려\s?줘|알려\s?주세요|계산해\s?줘|계산)$")
});
fn strip_tail(s: &str) -> String {
    let mut s = s.to_string();
    loop {
        let next = TAIL.replace(&s, "").trim().to_string();
        if next == s || next.is_empty() {
            return next;
        }
        s = next;
    }
}

const ASK_WORD: &str = r"며칠|몇\s*일|몇일|몇\s*날|날\s*수|일수|남은\s*날짜|남은\s*날|남은\s*기간|남았어?|남았나|남음|지났어?|지났나|지남|됐어?|되었어?|흘렀어?|째|몇\s*주|몇\s*달|몇\s*개월|기간|차이|며칠째|D-?day";
const AFTER: &str = r"(?:뒤|후|있다가|지나서|이후|뒤에|후에|지난\s*뒤|지난\s*후)";
const BEFORE: &str = r"(?:전|이전|전에)";
fn ask_days() -> String {
    format!(r"(?:\s*(?:{ASK_WORD}))*")
}

static R_TODAY: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:오늘|지금)\s*(?:날짜|며칠|몇\s*일|몇일|무슨\s*날)$"));
static R_TODAY_WD: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:오늘|지금)\s*(?:무슨|뭔|몇)\s*요일$"));
static R_WEEKNO: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:오늘|이번\s*주|지금)\s*(?:몇\s*주\s*차|몇\s*째\s*주|주차)$"));
static R_LEFT: LazyLock<Regex> = LazyLock::new(|| re(r"^(올해|이번\s*달|이번\s*주)\s*(?:남은\s*날|며칠\s*남았?|남은\s*기간|몇\s*일\s*남았?)$"));
static R_AGE: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:만\s*나이\s*(.+)|(.+?)\s*(?:생|태생|에\s*태어나면|출생)?\s*(?:만\s*)?나이)$"));
static R_DPM: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^d\s*([-+])\s*(\d{1,5})$"));
static R_DDAY1: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(?:디데이|d-?day)\s*(.+)$"));
static R_DDAY2: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)^(.+?)\s*(?:디데이|d-?day)$"));
static R_BIZ: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(r"^(?:(.+?)\s*(?:부터|로부터|으로부터|에서)\s*)?(?:(?:영업일|평일|근무일)\s*(\d{{1,4}})\s*일?|(\d{{1,4}})\s*(?:영업일|평일|근무일))\s*({AFTER}|{BEFORE}|째)?$"))
});
static R_BEFORE: LazyLock<Regex> = LazyLock::new(|| re(&format!("^{BEFORE}$")));
static R_RANGE_OP: LazyLock<Regex> = LazyLock::new(|| re(r"\s*(부터|에서|~|〜|–|\s-\s)\s*"));
static R_UNTIL_TAIL: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"\s*까지\s*{}$", ask_days())));
static R_SHIFT_OP: LazyLock<Regex> = LazyLock::new(|| re(r"\s*(\+|-|−|더하기|빼기|플러스|마이너스|부터|로부터|으로부터|에서|이후|기준)\s*"));
static R_SHIFT_TAIL: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"^(.+?)\s*({AFTER}|{BEFORE}|째\s*되는\s*날|째\s*날|째|되는\s*날)?$")));
static R_UNTIL: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"^(.+?)\s*까지\s*{}$", ask_days())));
static R_SINCE: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"^(.+?)\s*(?:부터|로부터|으로부터|이후)\s*(?:오늘\s*까지\s*)?{}$", ask_days())));
static R_FROM_TODAY: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(r"^(?:(?:오늘|지금)\s*(?:부터|로부터|으로부터|에서|기준)?\s*)?(.+?)\s*({AFTER}|{BEFORE}|째|째\s*되는\s*날|째\s*날)(?:\s*(?:날짜|언제|며칠|몇\s*일))?$"))
});
static R_WEEKDAY: LazyLock<Regex> = LazyLock::new(|| re(r"^(.+?)\s*(?:은|는|이|가)?\s*(?:무슨|뭔|몇)?\s*요일$"));
static R_SHORT: LazyLock<Regex> = LazyLock::new(|| re(r"^(\d{1,2}\s*[-./]\s*\d{1,2}|\d{1,2}\s*일|[일월화수목금토])$"));
static R_ISO_PLAIN: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{4}-\d{2}-\d{2}$"));

/// s 를 op 가 나오는 자리마다 앞 · 뒤로 나눠 f 에 — 처음 답이 나온 것 (2026-03-01 + 100일 의 "-" 에 속지 않게)
fn split_try(s: &str, op: &Regex, mut f: impl FnMut(&str, &str, &str) -> Option<Answer>) -> Option<Answer> {
    for c in op.captures_iter(s).flatten() {
        let m = c.get(0)?;
        let (left, right) = (s[..m.start()].trim(), s[m.end()..].trim());
        if !left.is_empty() && !right.is_empty() {
            if let Some(r) = f(left, right, g(&c, 1).unwrap_or(m.as_str().trim())) {
                return Some(r);
            }
        }
    }
    None
}

/// 쓴 글이 날짜 계산이면 답, 아니면 None
pub fn date_calc(text: &str, now: NaiveDateTime) -> Option<Answer> {
    let raw = text.trim();
    if raw.is_empty() || raw.contains('\n') || raw.chars().count() > 80 {
        return None;
    }
    let s = strip_tail(&squash(raw).replace('，', ","));
    if s.is_empty() {
        return None;
    }
    let s = s.as_str();
    let t = now.date();

    // --- 오늘 ---
    if is(&R_TODAY, s) || is(&R_TODAY_WD, s) {
        return Some(ans(day_text(t), iso_day(t), format!("올해 {}주차, 올해 {}번째 날", week_number(t), t.ordinal())));
    }
    if is(&R_WEEKNO, s) {
        return Some(ans(format!("{}주차", week_number(t)), week_number(t).to_string(), format!("{}, 월요일에 시작하는 주 (ISO)", day_text(t))));
    }
    if let Some(c) = caps(&R_LEFT, s) {
        let w = g(&c, 1)?;
        let end = if w == "올해" {
            mk(t.year(), 11, 31)?
        } else if w.starts_with("이번 달") || w == "이번달" {
            mk(t.year(), m0(t), last_day(t.year(), m0(t)))?
        } else {
            add_d(t, (7 - wd(t)) % 7)
        };
        let n = diff(t, end);
        return Some(ans(format!("{n}일 남음"), n.to_string(), format!("{}까지 (오늘 빼고), 오늘 포함 {}일", short_day(end, t), n + 1)));
    }

    // --- 나이 ---
    if let Some(c) = caps(&R_AGE, s) {
        let who = g(&c, 1).or(g(&c, 2))?;
        if let Some(born) = parse_day(who.strip_suffix('생').unwrap_or(who), now, Prefer::Past) {
            if born.explicit_year {
                return age_result(born.date, t);
            }
        }
    }

    // --- 시각 ---
    if let Some(r) = clock_calc(s, now) {
        return Some(r);
    }

    // --- D-100 · D+30 ---
    if let Some(c) = caps(&R_DPM, s) {
        let n: i64 = num(&c, 2);
        let date = add_d(t, n);
        let detail = if g(&c, 1) == Some("-") {
            format!("오늘이 D-{n}이면 D-day는 {n}일 뒤")
        } else {
            format!("오늘을 D-day로 치면 D+{n}, {n}일 뒤 (오늘을 1일째로 세면 {}일째)", n + 1)
        };
        return Some(ans(day_text(date), iso_day(date), detail));
    }

    // --- 디데이 12/25 · 12/25 디데이 ---
    if let Some(c) = caps(&R_DDAY1, s).or_else(|| caps(&R_DDAY2, s)) {
        if let Some(d) = parse_day(g(&c, 1)?, now, Prefer::Future) {
            return Some(until_result(d.date, t, ""));
        }
    }

    // --- 영업일 10일 뒤 · 평일 5일 후 · 2026-03-02부터 10영업일 뒤 ---
    if let Some(c) = caps(&R_BIZ, s) {
        let base = match g(&c, 1) {
            Some(b) => parse_day(b, now, Prefer::Near),
            None => Some(day(t)),
        };
        if let Some(base) = base {
            let n: i64 = g(&c, 2).or(g(&c, 3))?.parse().ok()?;
            let back = g(&c, 4).is_some_and(|w| is(&R_BEFORE, w));
            let nth = g(&c, 4) == Some("째");
            let start = if nth && !is_off(base.date) { add_d(base.date, -1) } else { base.date };
            let date = add_business_days(start, if back { -n } else { n });
            let from = if g(&c, 1).is_some() { format!("{}부터", short_day(base.date, t)) } else { "오늘부터".into() };
            let word = if back { " 전" } else if nth { "째" } else { " 뒤" };
            return Some(date_result(date, t, &format!("{from} 영업일 {n}일{word} (토 · 일 · 양력 공휴일 뺌)")));
        }
    }

    // --- 사이: A부터 B까지 · A ~ B ---
    let range = split_try(s, &R_RANGE_OP, |left, right, op| {
        let r = R_UNTIL_TAIL.replace(right, "").into_owned();
        if r == right && !op.contains(['~', '〜', '–', '-']) {
            return None; // "A부터 B" 는 "까지" 가 있어야 사이
        }
        let a = parse_day(left, now, Prefer::Near)?;
        let b = parse_day(&r, now, Prefer::Near)?;
        // 해를 안 쓴 끝 날이 앞 날보다 앞서면 다음 해로 (11월 1일 ~ 2월 1일)
        let mut end = b.date;
        if !b.explicit_year && !b.relative && end < a.date {
            end = end.with_year(end.year() + 1)?;
        }
        Some(range_result(a.date, end, t))
    });
    if range.is_some() {
        return range;
    }

    // --- 날짜 ± 기간: A + 100일 · A부터 100일 뒤 · A부터 100일째 · A - 2주 ---
    let shifted = split_try(s, &R_SHIFT_OP, |left, right, op| {
        let tail = caps(&R_SHIFT_TAIL, right)?;
        let base = parse_day(left, now, Prefer::Near)?;
        let dur = parse_duration(g(&tail, 1)?)?;
        if !dur.has_date() || dur.has_clock() {
            return None;
        }
        let word2 = g(&tail, 2);
        let sign = matches!(op, "+" | "-" | "−" | "더하기" | "빼기" | "플러스" | "마이너스");
        // "3월 1일부터 100일" 처럼 뒤 · 째 없이 날 수만: 그날을 1일째로 센 날 (기념일 세는 법)
        let bare = !sign && word2.is_none() && matches!(op, "부터" | "에서");
        if bare && dur.big() {
            return None;
        }
        let back = if sign { matches!(op, "-" | "−" | "빼기" | "마이너스") } else { word2.is_some_and(|w| is(&R_BEFORE, w)) };
        let nth = bare || word2.is_some_and(|w| w.starts_with('째'));
        if nth && dur.big() {
            return None; // "째" 는 날 수에만
        }
        let (date, clamped) = if nth { (add_d(base.date, dur.d as i64 - 1), false) } else { shift_date(base.date, &dur, if back { -1.0 } else { 1.0 }) };
        let word = if nth {
            format!("{}일째 (그날을 1일째로, {}일 뒤는 {})", dur.d, dur.d, short_day(add_d(base.date, dur.d as i64), t))
        } else {
            format!("{} {}", duration_text(&dur), if back { "전" } else { "뒤" })
        };
        let from = rel_word(diff(t, base.date)).map_or_else(|| short_day(base.date, t), String::from);
        Some(date_result(date, t, &format!("{from}부터 {word}{}", if clamped { ", 없는 날이라 그 달 말일" } else { "" })))
    });
    if shifted.is_some() {
        return shifted;
    }

    // --- 남은 날 · 지난 날 ---
    if let Some(c) = caps(&R_UNTIL, s) {
        let what = g(&c, 1)?.trim();
        if let Some(d) = parse_day(what, now, Prefer::Future) {
            return Some(until_result(d.date, t, if named(what).is_some() { what } else { "" }));
        }
    }
    if let Some(c) = caps(&R_SINCE, s) {
        if let Some(d) = parse_day(g(&c, 1)?, now, Prefer::Past) {
            let n = diff(d.date, t);
            if n >= 0 {
                let ymd = if n >= 28 { ymd_between(d.date, t) } else { String::new() };
                return Some(ans(
                    format!("{n}일 지남"),
                    n.to_string(),
                    join(&[&format!("{}부터 오늘까지", short_day(d.date, t)), &weeks_text(n), &ymd, &format!("그날을 1일째로 세면 오늘이 {}일째", n + 1)]),
                ));
            }
            return Some(until_result(d.date, t, ""));
        }
    }

    // --- 오늘에서: 3주 뒤 · 2달 전 · 100일째 · 열흘 뒤 ---
    if let Some(c) = caps(&R_FROM_TODAY, s) {
        if let Some(dur) = parse_duration(g(&c, 1)?) {
            if dur.has_date() && !dur.has_clock() {
                let w = g(&c, 2)?;
                if w.starts_with('째') {
                    if dur.big() {
                        return None;
                    }
                    return Some(date_result(add_d(t, dur.d as i64 - 1), t, &format!("오늘을 1일째로 세면 {}일째", dur.d)));
                }
                let back = is(&R_BEFORE, w);
                let (date, clamped) = shift_date(t, &dur, if back { -1.0 } else { 1.0 });
                let detail = format!("오늘부터 {} {}{}", duration_text(&dur), if back { "전" } else { "뒤" }, if clamped { ", 없는 날이라 그 달 말일" } else { "" });
                return Some(date_result(date, t, &detail));
            }
        }
    }

    // --- 요일: A 무슨 요일 · 다음 주 금요일 · 2026-12-25 (날짜만) ---
    if let Some(c) = caps(&R_WEEKDAY, s) {
        let what = g(&c, 1)?;
        if let Some(d) = parse_day(what, now, Prefer::Near) {
            return Some(date_result(d.date, t, ""));
        }
        if let Some(inner) = date_calc(what, now).filter(|a| is(&R_ISO_PLAIN, &a.plain)) {
            return Some(inner); // 100일 뒤 무슨 요일
        }
    }
    // 날짜만 쓴 것: 그날 요일과 남은 · 지난 날 ("12/25" 같은 짧은 꼴은 나눗셈과 겹쳐 계산기에 맡김)
    let only = parse_day(s, now, Prefer::Near)?;
    (!only.relative && !is(&R_SHORT, s)).then(|| date_result(only.date, t, ""))
}

/// 올해 몇 주차 (ISO: 월요일 시작)
fn week_number(d: D) -> u32 {
    d.iso_week().week()
}

fn age_result(born: D, t: D) -> Option<Answer> {
    if born > t {
        return None;
    }
    let had_birthday = (t.month(), t.day()) >= (born.month(), born.day());
    let age = t.year() - born.year() - if had_birthday { 0 } else { 1 };
    // 2월 29일생은 평년에 2월 28일을 생일로
    let bd = |y: i32| mk(y, m0(born), born.day()).unwrap_or_else(|| mk(y, 1, 28).unwrap());
    let mut next = bd(t.year());
    if next < t {
        next = bd(t.year() + 1);
    }
    let left = diff(t, next);
    let birthday = if left != 0 { format!("다음 생일까지 {left}일") } else { "오늘이 생일".into() };
    Some(ans(
        format!("만 {age}세"),
        age.to_string(),
        format!("{}생, {birthday}, 연 나이 {}세, 태어난 지 {}일", day_text(born), t.year() - born.year(), diff(born, t)),
    ))
}

static K_FROM_NOW: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"^(?:(?:지금|오늘|현재)\s*(?:부터|로부터|에서|기준)?\s*)?(.+?)\s*({AFTER}|{BEFORE})$")));
static K_SHIFT: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"^(.+?)\s*(?:(\+|-|−|더하기|빼기)|부터|에서)\s*(.+?)\s*({AFTER}|{BEFORE})?$")));
static K_RANGE: LazyLock<Regex> = LazyLock::new(|| re(r"^(.+?)\s*(?:부터|에서|~|〜|–)\s*(.+?)\s*(?:까지)?\s*(?:몇\s*시간|얼마|시간)?$"));
static K_MINUS: LazyLock<Regex> = LazyLock::new(|| re(r"^(.+?)\s+(-|−)\s+(.+)$"));

// 시각 계산: 3시간 뒤 · 오후 3시 + 2시간 · 9:15부터 18:30까지 · 14:30 - 9:15
fn clock_calc(s: &str, now: NaiveDateTime) -> Option<Answer> {
    let t = now.date();
    let show = |d: NaiveDateTime, detail: String| {
        let days = diff(t, d.date());
        let (lead, day_part) = if days != 0 { (format!("{} ", rel(days)), format!("{} ", iso_day(d.date()))) } else { (String::new(), String::new()) };
        ans(format!("{lead}{}", clock_text(d.hour() as i64, d.minute() as i64)), format!("{day_part}{:02}:{:02}", d.hour(), d.minute()), detail)
    };
    // 지금부터 N시간 뒤 · N분 전
    if let Some(c) = caps(&K_FROM_NOW, s) {
        if let Some(dur) = parse_duration(g(&c, 1)?) {
            if dur.has_clock() && dur.y == 0.0 && dur.mo == 0.0 {
                let sign = if is(&R_BEFORE, g(&c, 2)?) { -1.0 } else { 1.0 };
                let ms = ((dur.w * 7.0 + dur.d) * 1440.0 + dur.h * 60.0 + dur.min) * 60_000.0 + dur.s * 1000.0;
                let d = now + Duration::milliseconds((sign * ms) as i64);
                let word = if sign < 0.0 { "전" } else { "뒤" };
                return Some(show(d, format!("지금({})부터 {} {word}", clock_text(now.hour() as i64, now.minute() as i64), duration_text(&dur))));
            }
        }
    }
    // 시각 ± 기간: 오후 3시 + 2시간 30분 · 15:30 - 45분 · 9시부터 8시간 뒤
    if let Some(c) = caps(&K_SHIFT, s) {
        if let (Some(k), Some(dur)) = (parse_clock(g(&c, 1)?), parse_duration(g(&c, 3)?)) {
            if dur.has_clock() && !dur.big() {
                let back = match g(&c, 2) {
                    Some(op) => matches!(op, "-" | "−" | "빼기"),
                    None => g(&c, 4).is_some_and(|w| is(&R_BEFORE, w)),
                };
                let total = (k.h * 60 + k.min) as f64 + if back { -1.0 } else { 1.0 } * (dur.d * 1440.0 + dur.h * 60.0 + dur.min);
                let total = total.round() as i64;
                let shift = total.div_euclid(1440);
                let mins = total.rem_euclid(1440);
                let (h, m) = (mins / 60, mins % 60);
                let lead = if shift > 0 { "다음 날 " } else if shift < 0 { "전날 " } else { "" };
                let detail = format!("{}부터 {} {}", clock_text(k.h, k.min), duration_text(&dur), if back { "전" } else { "뒤" });
                return Some(ans(format!("{lead}{}", clock_text(h, m)), format!("{h:02}:{m:02}"), detail));
            }
        }
    }
    // 두 시각 사이: 9:15부터 18:30까지 · 14:30 - 9:15 · 오후 10시부터 오전 2시까지
    let (a, b) = if let Some(c) = caps(&K_RANGE, s) {
        (parse_clock(g(&c, 1)?), parse_clock(g(&c, 2)?))
    } else {
        let c = caps(&K_MINUS, s)?;
        (parse_clock(g(&c, 3)?), parse_clock(g(&c, 1)?))
    };
    let (mut a, mut b) = (a?, b?);
    // 오전 · 오후 없이 쓴 1~6시는 오후로 본다 (3시부터 5시 반까지 → 오후 3시 ~ 5시 반)
    if !a.ampm && (1..=6).contains(&a.h) {
        a.h += 12;
    }
    if !b.ampm && (1..=6).contains(&b.h) && (a.h >= 12 || b.h + 12 > a.h) {
        b.h += 12;
    }
    let am = a.h * 60 + a.min;
    let mut bm = b.h * 60 + b.min;
    // 12시간제로 쓴 끝이 앞서면 12시간 뒤로 (10시 ~ 2시 → 14시)
    if bm < am && !b.ampm && b.h < 12 && b.h + 12 > a.h {
        bm += 720;
    }
    let overnight = bm < am;
    if overnight {
        bm += 1440;
    }
    let total = bm - am;
    let (h, mm) = (total / 60, total % 60);
    let mut value = String::new();
    if h != 0 {
        value += &format!("{h}시간");
    }
    if h != 0 && mm != 0 {
        value += " ";
    }
    if mm != 0 || h == 0 {
        value += &format!("{mm}분");
    }
    let detail = format!("{} ~ {}{}, {total}분", clock_text(a.h, a.min), if overnight { "다음 날 " } else { "" }, clock_text(bm / 60 % 24, bm % 60));
    Some(ans(value, format!("{h}:{mm:02}"), detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 오늘을 2026-10-22 (목) 오후 2:10 으로 (메모킹 tests/date-calc.test.js 를 옮김)
    fn now() -> NaiveDateTime {
        D::from_ymd_opt(2026, 10, 22).unwrap().and_hms_opt(14, 10, 0).unwrap()
    }
    fn plain(q: &str) -> Option<String> {
        date_calc(q, now()).map(|a| a.plain)
    }
    fn value(q: &str) -> Option<String> {
        date_calc(q, now()).map(|a| a.value)
    }
    fn detail(q: &str) -> String {
        date_calc(q, now()).map(|a| a.detail).unwrap_or_default()
    }
    macro_rules! eq {
        ($f:ident, $q:expr, $want:expr) => {
            assert_eq!($f($q).as_deref(), Some($want), "{}", $q)
        };
    }

    #[test]
    fn from_today() {
        eq!(plain, "3주 뒤", "2026-11-12");
        eq!(plain, "100일 후", "2027-01-30");
        eq!(plain, "2달 전", "2026-08-22");
        eq!(plain, "3개월 뒤", "2027-01-22");
        eq!(plain, "1년 반 뒤", "2028-04-22");
        eq!(plain, "1.5년 뒤", "2028-04-22");
        eq!(plain, "반년 뒤", "2027-04-22");
        eq!(plain, "열흘 뒤", "2026-11-01");
        eq!(plain, "한 달 뒤", "2026-11-22");
        eq!(plain, "두 달 후", "2026-12-22");
        eq!(plain, "일주일 후", "2026-10-29");
        eq!(plain, "3일전", "2026-10-19");
        eq!(plain, "3일후", "2026-10-25");
        eq!(plain, "오늘로부터 100일 뒤", "2027-01-30");
        eq!(plain, "30일 뒤 날짜", "2026-11-21");
        eq!(value, "3주 뒤", "2026년 11월 12일 (목)");
        assert!(detail("3주 뒤").contains("오늘부터 3주 뒤, 21일 뒤"));
        assert!(!detail("100일 후").contains("100일 뒤, 100일 뒤"));
    }

    #[test]
    fn nth_and_dday() {
        eq!(plain, "100일째", "2027-01-29");
        eq!(plain, "오늘부터 100일", "2027-01-29");
        assert!(detail("오늘부터 100일").contains("100일 뒤는 2027년 1월 30일"));
        eq!(plain, "3월 1일부터 100일째", "2026-06-08");
        eq!(plain, "3월 1일부터 100일", "2026-06-08");
        eq!(plain, "D-100", "2027-01-30");
        eq!(plain, "d+30", "2026-11-21");
        assert!(detail("D-100").contains("D-day는 100일 뒤"));
    }

    #[test]
    fn shift_from_date() {
        eq!(plain, "2026-03-01 + 100일", "2026-06-09");
        eq!(plain, "12/25 - 2주", "2026-12-11");
        eq!(plain, "크리스마스로부터 30일 뒤", "2027-01-24");
        eq!(plain, "2026-01-31 + 1달", "2026-02-28");
        assert!(detail("2026-01-31 + 1달").contains("그 달 말일"));
        eq!(plain, "1월 31일 + 1개월", "2026-02-28");
        eq!(plain, "2028-02-29 + 1년", "2029-02-28");
    }

    #[test]
    fn until_and_since() {
        eq!(value, "12월 25일까지", "64일 남음");
        eq!(plain, "12/25까지 며칠", "64");
        eq!(plain, "12월 25일까지 며칠 남았어?", "64");
        eq!(plain, "2026년 12월 25일까지", "64");
        assert!(detail("12월 25일까지").contains("9주 1일, D-64"));
        eq!(plain, "크리스마스까지 몇 주", "64");
        eq!(plain, "어린이날까지 며칠이야", "195");
        eq!(plain, "내년 어린이날까지", "195");
        eq!(plain, "디데이 2027-01-01", "71");
        eq!(plain, "D-day 12/25", "64");
        eq!(plain, "12/25 디데이", "64");
        eq!(value, "금요일까지", "1일 남음");
        eq!(value, "2026-03-01부터", "235일 지남");
        assert!(detail("2026-03-01부터").contains("오늘이 236일째"));
        eq!(value, "어제부터 며칠", "1일 지남");
        assert_eq!(date_calc("13월 1일까지", now()), None);
        assert_eq!(date_calc("2/30까지", now()), None);
    }

    #[test]
    fn between() {
        eq!(plain, "1월 1일부터 6월 30일까지", "180");
        eq!(plain, "2026.1.1 ~ 2026.6.30", "180");
        assert!(detail("2026.1.1 ~ 2026.6.30").contains("25주 5일, 5개월 29일, 양 끝 포함 181일, 영업일 127일"));
        eq!(plain, "3월 1일부터 오늘까지 며칠", "235");
        eq!(plain, "3월 1일에서 오늘까지", "235");
        eq!(plain, "2026-03-01 ~ 오늘", "235");
        eq!(plain, "오늘부터 크리스마스까지", "64");
        eq!(plain, "11월 1일 ~ 2월 1일", "92");
    }

    #[test]
    fn business_days() {
        eq!(plain, "영업일 10일 뒤", "2026-11-05");
        eq!(plain, "평일 5일 후", "2026-10-29");
        eq!(plain, "10영업일 뒤", "2026-11-05");
        eq!(plain, "2026-03-02부터 10영업일 뒤", "2026-03-16");
        eq!(plain, "3월 1일부터 영업일 10일", "2026-03-13");
        eq!(plain, "2026-12-23부터 영업일 3일 뒤", "2026-12-29");
        assert!(detail("영업일 10일 뒤").contains("토 · 일 · 양력 공휴일 뺌"));
    }

    #[test]
    fn weekdays_and_named() {
        eq!(value, "12월 25일 무슨 요일", "2026년 12월 25일 (금)");
        eq!(plain, "2027-01-01 요일", "2027-01-01");
        eq!(plain, "다음 주 금요일", "2026-10-30");
        eq!(plain, "지난주 금요일", "2026-10-16");
        eq!(plain, "이번 주 토요일", "2026-10-24");
        eq!(plain, "2주 뒤 금요일", "2026-11-06");
        eq!(plain, "다음 달 말일", "2026-11-30");
        eq!(plain, "다음 달 15일", "2026-11-15");
        eq!(plain, "이번 달 말일", "2026-10-31");
        eq!(plain, "2026년 2월 말일", "2026-02-28");
        eq!(plain, "크리스마스 무슨 요일", "2026-12-25");
        eq!(plain, "100일 뒤는 무슨 요일이야?", "2027-01-30");
        eq!(plain, "2026-12-25", "2026-12-25");
        eq!(plain, "26.12.25", "2026-12-25");
        eq!(plain, "12월 25일", "2026-12-25");
        assert_eq!(date_calc("2027-02-29", now()), None);
    }

    #[test]
    fn clocks() {
        eq!(value, "3시간 뒤", "오후 5:10");
        eq!(plain, "90분 후", "15:40");
        eq!(plain, "1시간 반 뒤", "15:40");
        eq!(plain, "45분 전", "13:25");
        eq!(value, "12시간 뒤", "내일 오전 2:10");
        eq!(value, "2일 3시간 뒤", "모레 오후 5:10");
        eq!(value, "오후 3시 + 2시간", "오후 5:00");
        eq!(value, "오후 11시 + 3시간", "다음 날 오전 2:00");
        eq!(plain, "15:30 + 45분", "16:15");
        eq!(plain, "정오 + 90분", "13:30");
        eq!(value, "9:15부터 18:30까지", "9시간 15분");
        eq!(value, "14:30 - 9:15", "5시간 15분");
        eq!(value, "오후 10시부터 오전 2시까지", "4시간");
        eq!(value, "3시부터 5시 반까지", "2시간 30분");
        assert!(detail("3시부터 5시 반까지").contains("오후 3:00 ~ 오후 5:30"));
    }

    #[test]
    fn age_and_today() {
        eq!(value, "만 나이 1990-05-03", "만 36세");
        eq!(value, "1990년 5월 3일생 나이", "만 36세");
        eq!(value, "1990.05.03 만 나이", "만 36세");
        eq!(value, "만나이 2000-02-29", "만 26세");
        eq!(plain, "오늘 날짜", "2026-10-22");
        eq!(value, "오늘 무슨 요일", "2026년 10월 22일 (목)");
        eq!(value, "오늘 몇 주차", "43주차");
        eq!(value, "올해 남은 날", "70일 남음");
    }

    #[test]
    fn not_date_calc() {
        for q in ["내일 3시 회의", "3일 뒤 출장 준비", "견적서", "1,200 × 3", "12/25", "오늘", "25일", "금", "회의록 정리해줘", "3시 회의", "설날까지", "", "첫 줄\n3주 뒤"] {
            assert_eq!(date_calc(q, now()), None, "{q}");
        }
    }

    #[test]
    fn huge_numbers_do_not_panic() {
        for q in ["10000000년 뒤", "100000000일 뒤", "99999999999시간 뒤", "2026-01-01 + 99999999년", "9999999999999999999999999999일째", "오후 3시 + 99999999999분"] {
            assert_eq!(date_calc(q, now()), None, "{q}");
        }
        eq!(plain, "9999년 뒤", "12025-10-22");
    }

    #[test]
    fn pieces() {
        let t = now();
        assert_eq!(parse_day("12월 25일", t, Prefer::Past).unwrap().date.year(), 2025);
        assert_eq!(parse_day("12월 25일", t, Prefer::Future).unwrap().date.year(), 2026);
        assert_eq!(parse_day("3월 1일", t, Prefer::Future).unwrap().date.year(), 2027);
        assert_eq!(parse_day("2월 29일", t, Prefer::Future).unwrap().date.year(), 2028);
        assert_eq!(parse_duration("1년 2개월 3주 4일"), Some(Dur { y: 1.0, mo: 2.0, w: 3.0, d: 4.0, ..Dur::default() }));
        assert_eq!(parse_duration("2시간 반"), Some(Dur { h: 2.0, min: 30.0, ..Dur::default() }));
        assert_eq!(parse_duration("회의"), None);
        assert_eq!(parse_clock("오후 3시 30분"), Some(Clock { h: 15, min: 30, ampm: true }));
        assert_eq!(parse_clock("밤 12시"), Some(Clock { h: 12, min: 0, ampm: true }));
        assert_eq!(parse_clock("25:00"), None);
    }
}
