//! Multiple-entry visa calculator
//!
//! This program can be used to calculate the Schengen visa allowance (which applies to UK citizens
//! too) and plan trips according to the "90 out of 180 days" rule.

use anyhow::Result;

use chrono::{Datelike, Days, NaiveDate, Utc};
use clap::Parser;
use itertools::Itertools;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader};

const DATE_FMT: &str = "%Y-%m-%d";
const CONTROL_PERIOD_DAYS: usize = 180;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// End date for the visa calculation.
    #[arg(short, long)]
    end: Option<String>,

    /// File with dates in YYYY-MM-DD format. Dates should be ordered and should contain both the
    /// entry and exit dates for each interval.
    #[arg(short, long)]
    file: Option<String>,

    /// Number of days in the visa control period.
    #[arg(short, long, default_value_t = CONTROL_PERIOD_DAYS)]
    period: usize,
}

#[derive(Debug, Copy, Clone)]
struct DateInterval {
    a: NaiveDate,
    b: NaiveDate,
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

fn make_date_intervals(
    dates: &[NaiveDate],
    control_period: DateInterval,
) -> Result<Vec<DateInterval>> {
    let mut date_intervals = Vec::new();
    for (&a, &b) in dates.iter().tuples() {
        let mut di = DateInterval::new(a, b)?;
        if di.overlaps(control_period) {
            di.start_no_earlier(control_period.a);
            di.end_no_later(control_period.b);
            date_intervals.push(di);
        }
    }
    Ok(date_intervals)
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

    println!("Visa control period is {:?}", control_period);

    let dates = if let Some(filename) = cli.file {
        let mut file = OpenOptions::new().read(true).open(filename)?;

        parse_dates(BufReader::new(&mut file))
    } else {
        parse_dates(BufReader::new(io::stdin()))
    }?;

    let date_intervals = make_date_intervals(&dates, control_period)?;
    println!("Date intervals: {:?}", date_intervals);

    let total_days: usize = date_intervals.into_iter().map(|di| di.abs_num_days()).sum();

    println!("Days used in the control period: {}", total_days);

    Ok(())
}
