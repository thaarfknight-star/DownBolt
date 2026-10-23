// main.ts — نقطه‌ی ورود فرانت‌اند دانلودمنیجر (DownBolt)
import "./styles.css";
import { mountApp } from "./ui/app";

const root = document.getElementById("app");
if (root) mountApp(root);
