use kube::CustomResourceExt;

mod model;
fn main() {
    print!("{}", serde_yaml::to_string(&model::spec::Megaphone::crd()).unwrap())
}