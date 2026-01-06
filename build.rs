fn main() {
    // Only run ESP-IDF build for the esp32 feature
    #[cfg(feature = "esp32")]
    embuild::espidf::sysenv::output();
}
