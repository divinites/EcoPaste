import { useSnapshot } from "valtio";
import { CLIPBOARD_WINDOW_LAYOUT } from "@/constants/clipboardLayout";
import { useClipboardWindowEditableFocus } from "@/hooks/useClipboardWindowEditableFocus";
import { settingsState } from "@/stores/settings";
import Footer from "./components/Footer";
import Group from "./components/Group";
import Header from "./components/Header";
import List from "./components/List";

/**
 * 剪贴板窗口页面：按 `clipboard.window.layout` 在竖向列表与底部停靠横排之间切换渲染；
 * 窗口几何由 Rust 在每次显示前应用，这里只负责内容布局。
 */
const Clipboard = () => {
  const { clipboard } = useSnapshot(settingsState);
  const isDock = clipboard.window.layout === CLIPBOARD_WINDOW_LAYOUT.DOCK;

  useClipboardWindowEditableFocus();

  if (isDock) {
    return (
      <div
        className="flex size-screen flex-col overflow-hidden bg-ant-container"
        data-tauri-drag-region
      >
        <Header variant="dock" />

        <List orientation="horizontal" />

        <Footer />
      </div>
    );
  }

  return (
    <div
      className="flex size-screen flex-col overflow-hidden bg-ant-container"
      data-tauri-drag-region
    >
      <Header />

      <Group />

      <List />

      <Footer />
    </div>
  );
};

export default Clipboard;
