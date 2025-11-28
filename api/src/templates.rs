use tera::Tera;

lazy_static::lazy_static! {
    pub static ref TEMPLATES: Tera = {
        match Tera::new("api/templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Template parsing error: {}", e);
                std::process::exit(1);
            }
        }
    };
}
