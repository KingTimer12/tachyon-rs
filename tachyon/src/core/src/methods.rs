pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Method {
    pub fn new(id: u8) -> Self {
        match id {
            0 => Method::Get,
            1 => Method::Post,
            2 => Method::Put,
            3 => Method::Delete,
            4 => Method::Patch,
            _ => panic!("Invalid method ID"),
        }
    }

    pub fn id(&self) -> u8 {
        match self {
            Method::Get => 0,
            Method::Post => 1,
            Method::Put => 2,
            Method::Delete => 3,
            Method::Patch => 4,
        }
    }
}

impl PartialEq for Method {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl From<&str> for Method {
    fn from(method: &str) -> Self {
        match method {
            "0" => Method::Get,
            "1" => Method::Post,
            "2" => Method::Put,
            "3" => Method::Delete,
            "4" => Method::Patch,
            _ => panic!("Invalid method"),
        }
    }
}

impl From<&hyper::Method> for Method {
    fn from(method: &hyper::Method) -> Self {
        match *method {
            hyper::Method::GET => Method::Get,
            hyper::Method::POST => Method::Post,
            hyper::Method::PUT => Method::Put,
            hyper::Method::DELETE => Method::Delete,
            hyper::Method::PATCH => Method::Patch,
            _ => panic!("Invalid method"),
        }
    }
}

impl From<hyper::Method> for Method {
    fn from(method: hyper::Method) -> Self {
        match method {
            hyper::Method::GET => Method::Get,
            hyper::Method::POST => Method::Post,
            hyper::Method::PUT => Method::Put,
            hyper::Method::DELETE => Method::Delete,
            hyper::Method::PATCH => Method::Patch,
            _ => panic!("Invalid method"),
        }
    }
}
impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
        };
        write!(f, "{}", s)
    }
}
