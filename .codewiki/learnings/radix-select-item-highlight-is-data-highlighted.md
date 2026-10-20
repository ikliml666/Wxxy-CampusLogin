# Radix Select 的选项高亮是 data-highlighted，不是 DOM focus

## 症状

下拉选择器（适配器选择、运营商选择等所有基于 `components/ui/select.tsx` 的下拉）里，
鼠标移到某个选项上没有任何背景高亮；键盘上下键移动时同样没有。选项能正常点击选中，
只是「当前项」看不出来。

## 根因

`SelectItem` 的高亮类写的是 Tailwind 的 `focus:` 前缀：

```tsx
'... outline-none focus:bg-accent focus:text-accent-foreground ...'
```

但 Radix Select 的 `Item` **不会给选项元素 DOM focus**。它用 `data-highlighted` 标记
「键盘/鼠标当前指向的这一项」，实现在 `@radix-ui/react-select` 的 dist 里：

```js
"data-highlighted": isFocused ? "" : void 0,
```

即命中时属性存在且值为空串，未命中时属性完全不存在。`focus:` 变体要求元素是实际焦点
元素（`:focus` 伪类命中），而焦点始终在 `SelectContent` 的 listbox 容器上，所以
`focus:bg-accent` 永不匹配——高亮完全不生效，且不会有任何报错。

## 修复

改成属性选择器变体：

```tsx
'... outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground ...'
```

这也是 shadcn/ui 官方 Select 模板的写法。改动点：
`tauri-app/frontend/src/components/ui/select.tsx` 与
`android/frontend/src/components/ui/select.tsx`（安卓为独立复刻树，必须各改一份）。

## 验证方法（别再靠合成事件）

第一版探针用 `el.dispatchEvent(new PointerEvent('pointerover'))` 模拟悬停，
结论是**错的**——Radix 的 pointer 处理依赖真实指针状态（`hasPointerCapture`、
`pointerType`、内部的 pointerup 判定），合成事件不会把 `isFocused` 置真，
探针因此报告 `data-highlighted=null`，看起来「修复没生效」。

正确做法是用 CDP 的 `Input.dispatchMouseEvent` 发**真实**鼠标事件（`mousePressed` /
`mouseReleased` / `mouseMoved`，带坐标与 `button`），见 `.probe/select-hover-real.mjs`。
实测命中后 `data-highlighted=""`，浅色主题背景 `rgb(238,239,242)`，深色主题
`rgb(33,36,44)`，正是 `--accent` 的两套取值。

## 关联

- `--accent` 定义：`tauri-app/frontend/index.html` 的 `:root` / `.dark` 段与
  `src/index.css:19-20`（`--accent: 220 14% 94%`）。
- 同类属性选择器写法在 `components/ui/switch.tsx` 已有先例（`data-[state=checked]:*`），
  只是 select 的 Item 当初抄了旧模板的 `focus:`。
