use ironhold_core::start_app;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let project_path = args.get(1).cloned();
    start_app(project_path);
}
