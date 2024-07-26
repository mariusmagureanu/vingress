#[cfg(test)]
mod test {

    use crate::vcl::{update, Backend, UpdateError, Vcl};

    #[test]
    fn test_vcl_load() {
        let mut v = Vcl::new(
            String::from("default.vcl"),
            String::from("./template/vcl.hbs"),
        );

        let mut backends: Vec<Backend> = vec![];

        let b1 = Backend::new(String::from("alpha"), String::from("alpha.foo.com"), 8081);
        let b2 = Backend::new(String::from("beta"), String::from("beta.foo.com"), 8082);
        let b3 = Backend::new(String::from("delta"), String::from("delta.foo.com"), 8083);

        backends.push(b1);
        backends.push(b2);
        backends.push(b3);

        if let Some(e) = update(&mut v, backends) {
            assert!(e.to_string().len() > 0);
            assert_eq!(e, UpdateError::new(String::from("foo bar")));
        }

        assert!(v.content.len() > 0);
        println!("{:?}", v.content);
    }
}
