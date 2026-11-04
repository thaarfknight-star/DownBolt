// main.ts — نقطه‌ی ورود فرانت‌اند دانلودمنیجر (DownBolt)
import "./styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { mountApp } from "./ui/app";
import { mountDropbox } from "./ui/dropbox";

const root = document.getElementById("app");
if (root) {
  // پنجره‌ی جعبه‌ی شناور (لیبل dropbox) فقط رابط کوچک دراپ را می‌سازد؛
  // بقیه‌ی پنجره‌ها برنامه‌ی کامل را.
  if (getCurrentWindow().label === "dropbox") mountDropbox(root);
  else mountApp(root);
}
