import type { Metadata, Viewport } from "next";
import { DM_Sans, Geist_Mono } from "next/font/google";
import localFont from "next/font/local";
import "./globals.css";

const dmSans = DM_Sans({ variable: "--font-dm-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });
// DSEG by keshikan, SIL Open Font License 1.1 (app/fonts/DSEG-LICENSE.txt)
const dseg14 = localFont({ src: "./fonts/DSEG14Classic-Regular.woff2", variable: "--font-dseg14", display: "swap" });

export const metadata: Metadata = {
  title: "Sennheiser Control",
  description:
    "Control your Sennheiser headphones (MOMENTUM 4, ACCENTUM, MOMENTUM True Wireless and more) from the browser: noise control, EQ and headset settings over Bluetooth. Unofficial.",
};

export const viewport: Viewport = { themeColor: "#0b0c0f" };

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={`${dmSans.variable} ${geistMono.variable} ${dseg14.variable} h-full`}>
      <body className="min-h-full font-sans">{children}</body>
    </html>
  );
}
