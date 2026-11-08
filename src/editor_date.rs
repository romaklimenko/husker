use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};

use crate::query::parse_date;

pub(crate) fn parse_editor_date(value: &str, today: NaiveDate) -> Result<Option<NaiveDate>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let expanded = if value.bytes().all(|byte| byte.is_ascii_digit()) {
        match value.len() {
            8 => format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..]),
            6 => format!("20{}-{}-{}", &value[..2], &value[2..4], &value[4..]),
            4 => format!("{:04}-{}-{}", today.year(), &value[..2], &value[2..]),
            _ => value.to_owned(),
        }
    } else {
        value.to_owned()
    };
    if let Ok(date) = parse_date(&expanded) {
        return Ok(Some(date));
    }
    bail!("Invalid date '{value}'. Use YYYY-MM-DD, YYYYMMDD, YYMMDD, or MMDD.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Query;

    #[test]
    fn accepts_iso_and_compact_dates_without_changing_external_date_syntax() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let expected = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        for input in ["2026-11-09", "20261109", "261109", "1109"] {
            assert_eq!(parse_editor_date(input, today).unwrap(), Some(expected));
            assert_eq!(
                parse_editor_date(&format!(" \t{input}\n"), today).unwrap(),
                Some(expected)
            );
        }
        for input in ["20261109", "261109", "1109"] {
            assert!(parse_date(input).is_err());
            assert!(Query::parse(&format!("due:{input}")).is_err());
        }
        for input in ["", " \t\n"] {
            assert_eq!(parse_editor_date(input, today).unwrap(), None);
        }
    }

    #[test]
    fn short_years_always_use_2000_through_2099() {
        let today = NaiveDate::from_ymd_opt(2105, 1, 1).unwrap();
        for (input, expected) in [
            ("000229", "2000-02-29"),
            ("700101", "2070-01-01"),
            ("991231", "2099-12-31"),
            ("19991231", "1999-12-31"),
        ] {
            assert_eq!(
                parse_editor_date(input, today).unwrap(),
                Some(parse_date(expected).unwrap())
            );
        }
    }

    #[test]
    fn month_and_day_use_the_injected_year_without_rolling_forward() {
        let december = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let january = december.succ_opt().unwrap();
        assert_eq!(
            parse_editor_date("0101", december).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
        );
        assert_eq!(parse_editor_date("0101", january).unwrap(), Some(january));
        let normalized = parse_editor_date("0101", december)
            .unwrap()
            .unwrap()
            .to_string();
        assert_eq!(
            parse_editor_date(&normalized, january).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
        );
        assert!(parse_editor_date("0229", december).is_err());
        assert_eq!(
            parse_editor_date("0229", NaiveDate::from_ymd_opt(2028, 1, 1).unwrap()).unwrap(),
            Some(NaiveDate::from_ymd_opt(2028, 2, 29).unwrap())
        );
    }

    #[test]
    fn rejects_impossible_dates_non_ascii_digits_and_incomplete_input() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        for input in [
            "2026-02-29",
            "20260229",
            "260229",
            "0230",
            "0000",
            "1301",
            "1131",
            "2026-",
            "2026-1-09",
            "2026-01-9",
            "26-11-09",
            "11-09",
            "11/09",
            "110",
            "11090",
            "2026110",
            "202611090",
            "20 61109",
            "２６１１０９",
            "١١٠٩",
            "1109x",
            "+20261109",
        ] {
            let error = parse_editor_date(input, today).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("Invalid date '{input}'. Use YYYY-MM-DD, YYYYMMDD, YYMMDD, or MMDD.")
            );
        }
    }
}
