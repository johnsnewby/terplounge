use std::error::Error;
use std::fmt;

pub type E<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Clone)]
pub struct Er {
    msg: String,
}

impl Er {
    pub fn new(msg: String) -> Box<Er> {
        Box::new(Er { msg })
    }
}

impl std::error::Error for Er {}

impl fmt::Display for Er {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.msg)
    }
}
