// "일정 추가" 줄 — 쓴 글의 첫 줄을 메모킹 일정 탭 빠른 추가와 같은 규칙(memoking-core schedule_parse)으로 읽어
//   무엇이 저장될지 미리 보인다 ("내일 (토) 오후 3시 · 치과"). 저장은 메모킹이 같은 규칙으로 (--new-schedule-file)
use chrono::{DateTime, Datelike, Local, Timelike};
use memoking_core::schedule_parse::{parse_schedule_text, Schedule};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Preview {
    /// 언제 ("내일 (토) 오후 3시", "11월 20일 (금) ~ 22일 (일)", 여러 날이면 "… 외 2개")
    pub label: String,
    /// 일정 이름 (날짜 말을 뺀 글)
    pub title: String,
}

/// 날짜가 있고 이름이 있는 일정 글이면 미리보기 — 날짜를 못 찾았거나 이름이 비면 None (그때는 새 메모 줄만)
pub fn preview(text: &str, now: DateTime<Local>) -> Option<Preview> {
    let line = text.lines().next().unwrap_or("");
    let p = parse_schedule_text(line, None, now);
    let first = p.schedules.first().filter(|_| !p.needs_date_choice && !p.empty_title)?;
    let more = p.schedules.len() - 1;
    let label = format!("{}{}", when(first, now), if more > 0 { format!(" 외 {more}개") } else { String::new() });
    Some(Preview { label, title: p.title })
}

const WEEK: [&str; 7] = ["월", "화", "수", "목", "금", "토", "일"];

fn day(t: DateTime<Local>, now: DateTime<Local>) -> String {
    let wd = WEEK[t.weekday().num_days_from_monday() as usize];
    match (t.date_naive() - now.date_naive()).num_days() {
        0 => format!("오늘 ({wd})"),
        1 => format!("내일 ({wd})"),
        2 => format!("모레 ({wd})"),
        _ if t.year() != now.year() => format!("{}년 {}월 {}일 ({wd})", t.year(), t.month(), t.day()),
        _ => format!("{}월 {}일 ({wd})", t.month(), t.day()),
    }
}

fn clock(t: DateTime<Local>) -> String {
    let (h, m) = (t.hour(), t.minute());
    let half = if h < 12 { "오전" } else { "오후" };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    if m == 0 { format!("{half} {h12}시") } else { format!("{half} {h12}시 {m}분") }
}

fn when(s: &Schedule, now: DateTime<Local>) -> String {
    let at = |t: DateTime<Local>| if s.all_day { day(t, now) } else { format!("{} {}", day(t, now), clock(t)) };
    match s.end.filter(|_| s.range) {
        Some(end) => format!("{} ~ {}", at(s.start), at(end)),
        None if s.all_day => format!("{} 하루 종일", day(s.start, now)),
        None => at(s.start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn previews() {
        let now = Local.with_ymd_and_hms(2026, 11, 20, 10, 0, 0).unwrap(); // 금요일
        assert_eq!(preview("내일 오후 3시 치과", now), Some(Preview { label: "내일 (토) 오후 3시".into(), title: "치과".into() }));
        assert_eq!(preview("12월 3일 회의\n안건 정리", now).map(|p| p.title), Some("회의".into()));
        assert_eq!(preview("12월 3일 회의", now).map(|p| p.label), Some("12월 3일 (목) 하루 종일".into()));
        assert!(preview("11월 23일부터 25일까지 출장", now).is_some_and(|p| p.label.contains(" ~ ")));
        assert_eq!(preview("장보기 목록", now), None, "날짜가 없으면 일정 줄 없음");
        assert_eq!(preview("내일 3시", now), None, "이름이 없으면 일정 줄 없음");
    }
}
