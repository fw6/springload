import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/**
 * 版面用的字号不是 Tailwind 默认的 t-shirt 尺寸，tailwind-merge 会把它当成
 * 文字颜色，和 `text-ink` 这类颜色类相撞时丢掉其中一个。把自定义字号登记进去，
 * `cn` 才知道它们是尺寸而不是颜色。
 */
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [
        { text: ["micro", "note", "body", "lede", "head", "title", "masthead"] },
      ],
    },
  },
});

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
