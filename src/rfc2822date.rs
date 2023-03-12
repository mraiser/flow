use std::time::UNIX_EPOCH;
use std::time::SystemTime;

static DAY_NAMES:[&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
static DAYS_PER_MONTH: [i8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
static MONTH_NAMES:[&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

#[derive(Debug)]
pub struct RFC2822Date {
  pub year: i16,
  pub month: i8,
  pub day: i8,
  pub hour: i8,
  pub minute: i8,
  pub second: i8,
  pub millis: i16,
  pub weekday: String,
}

impl RFC2822Date {
  pub fn to_string(&self) -> String {
    let day: String = pad_two(self.day + 1);
    let month = MONTH_NAMES[self.month as usize];
    let hour = pad_two(self.hour);
    let minute = pad_two(self.minute);
    let second = pad_two(self.second);
    
    format!("{}, {} {} {} {}:{}:{} +0000", self.weekday, day, month, self.year, hour, minute, second)
  }

  pub fn now() -> RFC2822Date {
    let millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("error")
                    .as_millis()
                    .try_into()
                    .unwrap();
    RFC2822Date::new(millis)
  }
  
  pub fn new(millis: i64) -> RFC2822Date {
    let mut d = RFC2822Date {
      year: 0,
      month: 0,
      day: 0,
      hour: 0,
      minute: 0,
      second: 0,
      millis: 0,
      weekday: "".to_string(),
    };
    
    d.millis = (millis % 1000) as i16;
    let seconds = millis / 1000;
    d.second = (seconds % 60) as i8;
    let minutes = seconds / 60;
    d.minute = (minutes % 60) as i8;
    let hours = minutes / 60;
    d.hour = (hours % 24) as i8;
    
    let mut days = hours / 24;
    let day_of_week = days % 7;
    d.weekday = DAY_NAMES[day_of_week as usize].to_string();
    
    let mut year = 0;
    let mut leap;
    loop  {
      let num_days = num_days(year+1970) as i64;
      leap = num_days == 366;
      if days < num_days { break; }
      year += 1;
      days -= num_days;
    }
    d.year = year + 1970 as i16;
    
    let mut month = 0;
    loop {
      let mut num_days = DAYS_PER_MONTH[month] as i64;
      if month == 1 && leap { num_days += 1; }
      if days < num_days { break; }
      month += 1;
      days -= num_days;
    }
    d.month = month as i8;
    d.day = days as i8;
    
    d
  }
}

fn pad_two(i: i8) -> String {
  let mut s = i.to_string();
  if s.len() < 2 { s = "0".to_string() + &s; }
  s
}

fn num_days(year:i16) -> i16 {
  if year % 4 == 0 {
    if year % 400 == 0 || year % 100 != 0 {
      return 366;
    }
  }
  365
}

