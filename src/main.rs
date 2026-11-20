//! Firmware entry point: assemble the board, then dispatch to the selected app.
use anyhow::Result;
use std::time::Duration;
// use tiny_gate::apps::{self, demo}; // 测试模式下注释；恢复演示时解开
use tiny_gate::hardware::Hardware;
use tiny_gate::ui::Ui;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // 恢复演示：删掉下面的循环，解开顶部 use 与 match 分发
    // （分发会借用 &mut hw.button / &mut hw.rgb，届时把 hw 改回 let mut）。
    let hw = Hardware::probe();
    // 屏幕唯一移交点：Display 由 UI 层探测并独占整个固件周期，
    // 任何业务代码都拿不到驱动，只能经 Ui 更新画面。
    let mut ui = Ui::new();
    boot_page(&mut ui);
    hw.log_system()?;

    // BOOT 页面测试模式：开机页画完后停帧观察，不进入任何 app。
    // main 不返回 → hw 与 ui 一直活着，画面自然保持（park 形态，无需 forget）。
    // 恢复演示：删掉下面的循环，解开顶部 use 与 match 分发。
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }

    // match apps::select() {
    //     apps::App::CoexistBench => apps::coexist::run(&mut ui, &mut hw.button),
    //     apps::App::ConnectivityBench => {
    //         apps::connectivity::run(&mut hw.rgb, &mut hw.button)
    //     }
    //     apps::App::ZigbeeDemo => {
    //         demo::zigbee_demo(&mut ui, &mut hw.button, &mut hw.rgb)
    //     }
    //     apps::App::RadioBench => demo::radio_bench(&mut ui, &mut hw),
    // }
}

/// Boot page on the OLED before the selected app takes over the board.
///
/// Draws the fixed status-page frame: top bar (radio icons + label) on
/// row 0, project name on row 2. A missing display skips the page without
/// blocking the firmware.
fn boot_page(ui: &mut Option<Ui>) {
    let Some(ui) = ui.as_mut() else {
        return;
    };
    let _ = ui.update_and_fmt_top_bar(true, true, true, "12:34");
    let _ = ui.update_status("TINYGATE", 1);
}
