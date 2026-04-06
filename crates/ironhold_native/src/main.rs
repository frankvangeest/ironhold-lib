use ironhold_core::start_app;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut project_name: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--project" if i + 1 < args.len() => {
                project_name = Some(args[i + 1].clone());
                i += 2;
            }
            _ => { i += 1; }
        }
    }
    let project_path = project_name
        .map(|name| format!("projects/{}/{}.project.ron", name, name));
    start_app(project_path, None);
}
