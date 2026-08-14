use downloader::error::{Error, ErrorKind};

fn main() {
    let e = Error::new(ErrorKind::Parse, "test".to_string());
    println!("{:?}", e);

}

