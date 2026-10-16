// Planning prod : app de bureau (Mac / Windows / Linux)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() { planning_prod_lib::run() }
