fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/glossa-icon-2.ico");
        res.compile().unwrap();
    }
}
