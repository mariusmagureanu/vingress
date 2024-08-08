#[cfg(test)]
mod test {

    use crate::vcl::{update, Backend, Vcl};
    use std::{fs::File, io::Read};

    #[test]
    fn test_vcl_load() {
        let mut v = Vcl::new("default.vcl", "./template/vcl.hbs", ".");

        let mut backends: Vec<Backend> = vec![];

        let b1 = Backend::new(
            String::from("foo"),
            String::from("alpha"),
            String::from("alpha.foo.com"),
            "/".to_string(),
            String::from("service1"),
            String::from("Prefix"),
            8081,
        );
        let b2 = Backend::new(
            String::from("foo"),
            String::from("beta"),
            String::from("beta.foo.com"),
            "/foo".to_string(),
            String::from("service2"),
            String::from("Exact"),
            8082,
        );
        let b3 = Backend::new(
            String::from("foo"),
            String::from("delta"),
            String::from("delta.foo.com"),
            "/bar".to_string(),
            String::from("service3"),
            String::from("ImplementationSpecific"),
            8083,
        );

        backends.push(b1);
        backends.push(b2);
        backends.push(b3);

        if let Some(e) = update(&mut v, backends) {
            panic!("{}", e);
        }

        match File::open("default.vcl") {
            Ok(mut vf) => {
                let mut vcl_content_from_file: String = Default::default();
                let _ = vf.read_to_string(&mut vcl_content_from_file);

                assert_eq!(v.content.len(), vcl_content_from_file.len());
            }
            Err(e) => panic!("{}", e),
        }
    }
}
