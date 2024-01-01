//! Multiple-entry visa calculator
//!
//! This program can be used to calculate the Schengen visa allowance (which applies to UK citizens
//! too) and plan trips according to the "90 out of 180 days" rule.

use anyhow::Result;
use chrono::{Datelike, Days, NaiveDate, Utc};
use clap::Parser;
use itertools::Itertools;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader};

const DATE_FMT: &str = "%Y-%m-%d";
const CONTROL_PERIOD_DAYS: usize = 180;
const ALLOWED_DAYS: usize = 90;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// End date for the visa calculation. Defaults to today's date.
    #[arg(short, long)]
    end: Option<String>,

    /// File with dates in YYYY-MM-DD format. Dates should contain both the entry and exit dates for
    /// each interval.
    #[arg(short, long)]
    file: Option<String>,

    /// Number of days in the visa control period.
    #[arg(short, long, default_value_t = CONTROL_PERIOD_DAYS)]
    period: usize,

    /// Maximum number of days allowed.
    #[arg(short, long, default_value_t = ALLOWED_DAYS)]
    allowed: usize,
}

#[derive(Debug, Copy, Clone)]
struct DateInterval {
    a: NaiveDate,
    b: NaiveDate,
}

impl fmt::Display for DateInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "from {} to {}", self.a, self.b)
    }
}

impl DateInterval {
    /// Makes a new date interval, checking that the interval points are not reversed.
    pub fn new(a: NaiveDate, b: NaiveDate) -> Result<Self> {
        if a > b {
            anyhow::bail!("End date is before the start date");
        }
        Ok(Self { a, b })
    }

    /// Calculates the number of days in the interval including the last day.
    pub fn abs_num_days(&self) -> usize {
        (self.b - self.a).num_days() as usize + 1
    }

    /// Updates the start date of the interval to the given date `d` if the interval starts before
    /// `d` and ends on `d` or after.
    pub fn start_no_earlier(&mut self, d: NaiveDate) {
        if self.a < d && d <= self.b {
            self.a = d;
        }
    }

    pub fn end_no_later(&mut self, d: NaiveDate) {
        if self.a <= d && d < self.b {
            self.b = d;
        }
    }

    pub fn overlaps(&self, di: DateInterval) -> bool {
        self.a <= di.b && di.a <= self.b
    }
}

struct DateIntervalVec(Vec<DateInterval>);

impl DateIntervalVec {
    fn from_dates(dates: &[NaiveDate], control_period: DateInterval) -> Result<Self> {
        let mut date_intervals = Vec::new();
        for (&a, &b) in dates.iter().tuples() {
            let mut di = DateInterval::new(a, b)?;
            if di.overlaps(control_period) {
                di.start_no_earlier(control_period.a);
                di.end_no_later(control_period.b);
                date_intervals.push(di);
            }
        }
        Ok(Self(date_intervals))
    }

    fn num_spent_days(&self) -> usize {
        let spent_days: usize = self.0.iter().map(|di| di.abs_num_days()).sum();
        spent_days
    }
}

impl fmt::Display for DateIntervalVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut n = 1;
        let mut iter = self.0.iter().peekable();
        while let Some(di) = iter.next() {
            write!(
                f,
                "{n}) {di}{}",
                if iter.peek().is_some() { ", " } else { "" }
            )?;
            n += 1;
        }
        Ok(())
    }
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(s, DATE_FMT)?)
}

fn parse_dates<R: BufRead>(mut reader: R) -> Result<Vec<NaiveDate>> {
    let mut dates = Vec::new();
    loop {
        let mut buffer = String::new();
        let bytes = reader.read_line(&mut buffer)?;

        if bytes == 0 {
            // EOF reached
            return Ok(dates);
        } else {
            let date = parse_date(buffer.trim())?;
            dates.push(date);
        }
    }
}

fn sort_and_dedup_dates(dates: &mut Vec<NaiveDate>) {
    dates.sort();
    let num_dates = dates.len();
    dates.dedup();
    let num_dups = num_dates - dates.len();
    if num_dups > 0 {
        println!(
            "WARNING: {num_dups} duplicate date{} found and removed",
            if num_dups > 1 { "s" } else { "" }
        );
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let control_period = Days::new(180);
    let end_date = if let Some(date) = cli.end {
        parse_date(&date)?
    } else {
        let now = Utc::now();
        if let Some(date) = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()) {
            date
        } else {
            anyhow::bail!("Bad now");
        }
    };
    let start_date = end_date - control_period;
    let control_period = DateInterval::new(start_date, end_date)?;

    println!("Visa control period is {control_period}");

    let mut dates = if let Some(filename) = cli.file {
        let mut file = OpenOptions::new().read(true).open(filename)?;

        parse_dates(BufReader::new(&mut file))
    } else {
        parse_dates(BufReader::new(io::stdin()))
    }?;

    sort_and_dedup_dates(&mut dates);

    let date_intervals = DateIntervalVec::from_dates(&dates, control_period)?;
    println!("Date intervals: {}", date_intervals);

    let num_spent_days = date_intervals.num_spent_days();
    println!("Days spent in the control period: {num_spent_days}");
    println!(
        "Spent days {} the allowed number",
        if num_spent_days > cli.allowed {
            "exceed"
        } else {
            "are within"
        }
    );

    Ok(())
}
