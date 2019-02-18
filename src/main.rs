extern crate colored;
extern crate yaml_rust;
extern crate hyper;
extern crate hyper_native_tls;
extern crate time;
extern crate csv;
extern crate regex;
extern crate clap;
extern crate serde_json;

mod config;
mod interpolator;
mod benchmark;
mod reader;
mod checker;
mod writer;
mod actions;
mod expandable;

use self::clap::{Arg, App};
use colored::*;
use std::process;
use std::collections::HashMap;
use std::f64;
use clap::crate_version;
use actions::Report;

fn main() {
  let matches = app_args();
  let benchmark_file = matches.value_of("benchmark").unwrap();
  let report_path_option = matches.value_of("report");
  let stats_option = matches.is_present("stats");
  let compare_path_option = matches.value_of("compare");
  let threshold_option = matches.value_of("threshold");
  let no_check_certificate = matches.is_present("no-check-certificate");

  let begin = time::precise_time_s();
  let list_reports_result = benchmark::execute(benchmark_file, report_path_option, no_check_certificate);
  let duration = time::precise_time_s() - begin;

  match list_reports_result {
    Ok(list_reports) => {
      show_stats(&list_reports, stats_option, duration);
      compare_benchmark(&list_reports, compare_path_option, threshold_option);

      process::exit(0)
    },
    Err(_) => process::exit(1),
  }
}

fn app_args<'a> () -> clap::ArgMatches<'a> {
  return App::new("drill")
    .version(crate_version!())
    .about("HTTP load testing application written in Rust inspired by Ansible syntax")
    .arg(Arg::with_name("benchmark")
                .help("Sets the benchmark file")
                .long("benchmark")
                .short("b")
                .required(true)
                .takes_value(true))
    .arg(Arg::with_name("stats")
                .short("s")
                .long("stats")
                .help("Shows request statistics")
                .takes_value(false)
                .conflicts_with("compare"))
    .arg(Arg::with_name("report")
                .short("r")
                .long("report")
                .help("Sets a report file")
                .takes_value(true)
                .conflicts_with("compare"))
    .arg(Arg::with_name("compare")
                .short("c")
                .long("compare")
                .help("Sets a compare file")
                .takes_value(true)
                .conflicts_with("report"))
    .arg(Arg::with_name("threshold")
                .short("t")
                .long("threshold")
                .help("Sets a threshold value in ms amongst the compared file")
                .takes_value(true)
                .conflicts_with("report"))
    .arg(Arg::with_name("no-check-certificate")
                .long("no-check-certificate")
                .help("Disables SSL certification check. (Not recommended)")
                .takes_value(false))
    .get_matches();
}

fn show_stats (list_reports: &Vec<Vec<Report>>, stats_option: bool, duration: f64) {
  if !stats_option { return }

  let mut group_by_status = HashMap::new();

  for req in list_reports.concat() {
    group_by_status.entry(req.status / 100).or_insert(Vec::new()).push(req);
  }

  let durations = list_reports.concat().iter().map(|r| r.duration).collect::<Vec<f64>>();
  let mean = durations.iter().fold(0f64, |a, &b| a + b) / durations.len() as f64;
  let deviations = durations.iter().map(|a| (mean - a).powf(2.0)).collect::<Vec<f64>>();
  let stdev = (deviations.iter().fold(0f64, |a, &b| a + b) / durations.len() as f64).sqrt();

  let mut sorted = durations.clone();
  sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
  let durlen = sorted.len();
  let median = if durlen % 2 == 0 {
    sorted[durlen / 2]
  } else {
    (sorted[durlen / 2] + sorted[durlen / 2 + 1]) / 2f64
  };

  let pct90 = percentile(90, &sorted);
  let pct99 = percentile(99, &sorted);

  let total_requests = list_reports.concat().len();
  let successful_requests = group_by_status.entry(2).or_insert(Vec::new()).len();
  let failed_requests = total_requests - successful_requests;
  let requests_per_second = total_requests as f64 / duration;

  println!("");
  println!("{} {}", "Concurrency Level".yellow(), list_reports.len().to_string().purple());
  println!("{} {} {}", "Time taken for tests".yellow(), format!("{:.1}", duration).to_string().purple(), "seconds".purple());
  println!("{} {}", "Total requests".yellow(), total_requests.to_string().purple());
  println!("{} {}", "Successful requests".yellow(), successful_requests.to_string().purple());
  println!("{} {}", "Failed requests".yellow(), failed_requests.to_string().purple());
  println!("{} {} {}", "Requests per second".yellow(), format!("{:.2}", requests_per_second).to_string().purple(), "[#/sec]".purple());
  println!("{} {}{}", "Median time per request".yellow(), median.round().to_string().purple(), "ms".purple());
  println!("{} {}{}", "Average time per request".yellow(), mean.round().to_string().purple(), "ms".purple());
  println!("{} {}{}", "Sample standard deviation".yellow(), stdev.round().to_string().purple(), "ms".purple());
  println!("{} {}{}", "Percentile 90th".yellow(), pct90.round().to_string().purple(), "ms".purple());
  println!("{} {}{}", "Percentile 99th".yellow(), pct99.round().to_string().purple(), "ms".purple());
}

fn percentile (p: u8, sorted_durations: &Vec<f64>) -> f64 {
  assert!(p > 0 && p <= 100);

  let ordinal_rank: f64 = (p as f64) / 100.0 * (sorted_durations.len() as f64);
  let ordinal_rank = ordinal_rank.ceil() as usize;
  return sorted_durations[ordinal_rank - 1];
}

fn compare_benchmark (list_reports: &Vec<Vec<Report>>, compare_path_option: Option<&str>, threshold_option: Option<&str>) {
  if let Some(compare_path) = compare_path_option {
    if let Some(threshold) = threshold_option {
      let compare_result = checker::compare(&list_reports, compare_path, threshold);

      match compare_result {
        Ok(_) => process::exit(0),
        Err(_) => process::exit(1),
      }
    } else {
      panic!("Threshold needed!");
    }
  }
}
