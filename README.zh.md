# handlist

[English](README.md) | **简体中文**

**为人工编辑而设计的数据描述语言**：整体是 YAML 的子集，保留 YAML 易读的块状结构，去掉所有依赖隐式规则、容易写错的语法。扩展名为 `.handlist`。

最初用于描述网站的目录与页面配置，也适用于菜单、索引、文档元数据等需要大量手写的结构化数据。

- 规范：[`spec/handlist-spec.md`](spec/handlist-spec.md)（英文，1.0）
- Rust 实现：[`rust/`](rust/)（解析 + 保留注释的格式化 + JSON Schema 校验 + 规范序列化 + serde + `handlist` check/fmt CLI）
- TypeScript 实现：[`typescript/`](typescript/)（零运行时依赖，ESM/CJS + 类型）
- 示例：[`examples/`](examples/)

---

## 设计目标

1. **结构只看缩进。** 固定每层 2 个空格，禁止 YAML 的“缩进序列”（`key:` 下一行 `-` 与键同列），禁止任意宽度缩进。
2. **每种东西只有一种写法。** 字符串只有普通标量和双引号两种；没有单引号字符串、折叠块 `>`、锚点、别名、标签、flow mapping。
3. **没有隐式惊喜。** `key:` 不表示 null；`yes`、`1e3`、`0x10`、`01` 都是字符串；映射键永远是字符串；重复键是错误。
4. **多行文本保留换行。** 用 `|` 编写 HTML 或长段落，内容逐行保留，末尾不带换行。
5. **机器友好。** 数据模型对应 JSON 类值；Rust 提供完整 serde 支持；规范序列化可稳定往返。
6. **严格 YAML 1.2 语法子集。** 任何合法 handlist 文件都能被 YAML 解析器读取；可以用 YAML 高亮、编辑器与校验器。但值语义按 handlist 的规则解释。

## 语法一览

```handlist
# 站点索引
site:
  name: Example
  enabled: true
  pages:
    - name: home
      title: 首页
      action: page
    - name: tools
      title: 工具
      action: page
      tags: [hardware, info]
  description: |
    第一行
    第二行 <b>HTML</b>
```

### 结构

- 缩进固定为 2 个空格；子块必须正好比父层多 2 格。
- 映射：`key: value`；如果值在下一行，下一行必须是映射或序列（`key:` 后什么都不跟不是 null）。
- 序列：`- value`，`-` 后必须恰好一个空格；支持 `- key: value` 紧凑映射与 `- - value` 嵌套序列。
- 注释：整行注释 `# 任意内容`；行尾注释写成 ` # 注释`（`#` 后有空格），`#` 后直接换行则表示空注释；`C#`、URL 里的 `#` 是普通内容。
- 块文本：`key: |` 或 `- |`，内容比父层多缩进 2 格；逐行保留，去掉每行行尾空白与首尾空行，值末尾无换行。
- 行内数组：`[a, b, c]`，只允许单行标量元素；空数组 `[]`、空映射 `{}`。
- 映射键总是字符串；裸键限 `[A-Za-z0-9_][A-Za-z0-9_-]*`，其他字符用双引号键。`10:` 和 `"10":` 等价。
- 标量类型：只识别 `true`/`false`/`null`、十进制整数（无前导零、安全整数范围内）、带小数点的小数（无指数）。其余全是字符串。
- 文档：单文档、单根节点；不支持 YAML 文档标记（`---`/`...`）、`%YAML`、锚点、别名、标签、多文档。

这些写法会被明确拒绝：`yes`/`no` 作为布尔、`key:` 隐式 null、缩进序列、`>` 折叠块、`|-`/`|+`、单引号字符串、非空 flow mapping、尾逗号、行内 `#注释`（无空格）、Tab 缩进、重复键、多个根节点。完整规则见规范。

## 仓库结构

```text
.
├── spec/
│   └── handlist-spec.md          规范（EBNF、语义、规范化、错误、YAML 对比）
├── rust/
│   ├── src/                      解析器、span、格式化、JSON Schema、序列化、serde
│   │   └── bin/handlist.rs       check/fmt CLI
│   ├── tests/                    语法、span、serde、格式化、schema、CLI、示例
│   └── examples/canonicalize.rs  打印/生成 canonical 输出
├── typescript/
│   ├── src/                      零依赖 TS 实现
│   ├── test/                     语法、序列化、示例、YAML 子集校验
│   └── dist/                     构建产物（ESM/CJS/类型）
└── examples/
    ├── homepage/                 8 个从 YAML 改写的 .handlist 示例
    └── canonical/                对应的 canonical 输出（Rust/TS 共用契约）
```

## Rust

```toml
[dependencies]
handlist = { path = "rust" }
```

```rust
use handlist::Value;

fn main() -> Result<(), handlist::Error> {
    let source = r#"
site:
  name: Example
  pages:
    - name: home
      title: 首页
"#;

    let value: Value = handlist::parse(source)?;
    println!("{}", handlist::to_string_value(&value)?);
    Ok(())
}
```

主要 API：

- `handlist::parse(&str) -> Result<Value, Error>`：解析为有序 `Value`。
- `handlist::parse_with_spans(&str) -> Result<(Value, SpanMap), Error>`：解析并返回 JSON Pointer 位置映射（`Span { line, column }`）。`SpanMap::get(pointer)` 是值的起始位置，`SpanMap::key_span(pointer)` 是拥有该值的映射键的起始位置；指针按 RFC 6901 转义。
- `handlist::to_string_value(&Value) -> Result<String, Error>`：canonical 序列化，始终可用。
- `handlist::format(&str) -> Result<String, Error>`（feature `format`）：canonical 源码格式化，并保留注释。整行注释随所属节点重新缩进；行内注释尽可能保留在值所在行。
- `handlist::format_with_config(&str, &FormatConfig)` 与 `FormatConfig::parse(&str)`（feature `format`）：应用 `.fmt.handlist` 格式化配置。可选的 `non-complex-one-line-seq` 会把源码中元素均为非复合值的单行 flow sequence 保留在一行；`one-line-in-multi-line-text` 会把源码中内容为 0 或 1 行的 `|` 块文本保留为 block 形式。两者只影响呈现，不改变值。
- `handlist::validate_json_schema(&str, &str) -> Result<SchemaValidation, Error>`（feature `schema`）：将文档转换为 JSON 后按 schema 校验。提供 `SchemaValidation::{is_valid, errors}` 与 `SchemaError::{instance_path, schema_path, message}`。支持 draft-07 和 2020-12；支持内部 `$ref`，不解析外部引用；没有 `$schema` 时按 2020-12 处理。
- `Value::{Null, Bool, Int, Float, String, Seq, Map}`；`Map` 保留插入顺序。
- `Error::{message, span, line, column}`；位置从 1 开始，列按 Unicode 标量值计数。
- 默认启用 `serde` feature：
  - `handlist::from_str::<T>(&str)`
  - `handlist::to_string(&T)`
  - `handlist::to_value(&T)`
  - `Value`/`Map` 自身实现 `Serialize`/`Deserialize`。
- `from_str` 内部按带 span 的方式解析，因此 serde 错误（字段类型错误、缺字段、未知字段、序列元素类型错误、映射键错误）会带上当前正在解码的字段位置。缺字段错误使用最后消费的键；映射为空时回退到容器/根节点。
- 数字型映射键也能从字符串键正确反序列化，例如 `BTreeMap<u8, String>` 读取 `10: ...`。
- 默认只启用 `serde`。`format` 启用 `format`、`format_with_config` 与 `FormatConfig`；`schema` 启用 `validate_json_schema`；`cli` 启用 `handlist` 二进制，并同时启用 `schema` 和 `format`。`parse`、`parse_with_spans`、`Value` 与 `to_string_value` 无需可选 feature。只做解析时使用 `default-features = false`（需要类型反序列化时保留 `serde`）。

### CLI

启用 `cli` feature 后构建 `handlist` 二进制。在 `rust/` 目录下运行：

```bash
cargo run --features cli --bin handlist -- check ../examples
cargo run --features cli --bin handlist -- check config.handlist --schema schema.json
cargo run --features cli --bin handlist -- fmt --check config.handlist
cargo run --features cli --bin handlist -- fmt config.handlist
```

安装 CLI 使用 `cargo install --path rust --features cli`。

`fmt` 会在当前工作目录查找 `.fmt.handlist`，也可以用 `--config PATH` 指定。配置本身也是 handlist 文档：

```handlist
version: 1
allow:
  - non-complex-one-line-seq
  - one-line-in-multi-line-text
```

`non-complex-one-line-seq` 只保留源码中已经写成一行、且元素都是标量的 flow sequence，不会把 block sequence 折叠成一行。`one-line-in-multi-line-text` 只保留源码中内容为 0 或 1 行的 `|` 块文本，不会把 inline 字符串展开成 block。配置采用严格校验：`version` 必须为 `1`（`allow` 可以省略），未知字段/选项和重复选项都会报错。

- `check` 检查文件或目录。显式传入的文件不检查扩展名；目录会递归查找文件名以 `.handlist` 结尾的文件。使用 `--schema` 时，将转为 JSON 的文档按 draft-07 或 2020-12 JSON Schema 校验；不传时只检查语法。
- `fmt` 默认就地写入 canonical 格式。`--check` 只报告差异、不写入文件，并在需要改动时退出 `1`。注释会保留：整行注释随所属节点重新缩进，行内注释尽可能留在值所在行；如果 flow sequence 被展开为 block sequence，其行内注释会变成该值之后的整行注释。注释附近的空行和顶层节点之间的空行最多保留一个。
- 两个命令在未提供路径时检查 `.`，默认跳过 `.git`、`target`、`node_modules`，`-a`/`--all` 会包含它们；递归时不跟随目录符号链接。诊断格式为 `path:line:column: error: ...`，失败时退出码为 `1`。
- 参数解析使用 [`argh`](https://crates.io/crates/argh)。

测试：

```bash
cd rust
cargo test --all-features
```

## TypeScript

```bash
cd typescript
npm install
npm run build
```

```ts
import {
  parse,
  parseOrdered,
  stringify,
} from "handlist";

const config = parse(source);            // 映射是普通对象，便于 JSON 使用
const ordered = parseOrdered(source);    // 映射是 Map，严格保留源顺序

console.log(stringify(config));          // canonical handlist 文本
```

- 零运行时依赖，Node.js >= 18；同时输出 ESM、CJS 和 `.d.ts`。
- `parse()` 返回 JSON 友好的普通对象；`parseOrdered()` 返回 `Map`，键顺序（包括 `10`、`2`、`1` 这类数字样键）严格按源文件。
- `stringify()` 接受普通对象、`Map`、数组和标量，输出规范文本。
- 解析错误抛出 `HandlistError`，带 `reason`、`line`、`column`。

**两个 JavaScript 主机语言限制**：

1. 普通对象的数字样键会被 JavaScript 排序（`"10"`、`"2"`、`"1"` 变成 `"1"`、`"2"`、`"10"`）。需要精确顺序时用 `parseOrdered()`（返回 `Map`），或使用 Rust 实现。
2. JavaScript 只有一种 `number` 类型，无法区分整数与浮点。`parseOrdered()` 对 `1.0` 与 `1` 都返回数值 `1`；`stringify()` 会把安全整数范围内的整数值按整数形式输出，更大的整数值按浮点形式输出。需要严格区分 int/float 时使用 Rust 实现。

测试：

```bash
cd typescript
npm test
```

测试会用 [`yaml`](https://www.npmjs.com/package/yaml) 包校验所有 `.handlist` 示例与 canonical fixture 同时也是合法 YAML 1.2。

## 示例

[`examples/homepage/`](examples/homepage/) 里的 8 个文件来自 [`ldtstore-homepage/page`](https://github.com/stackinspector/ldtstore-homepage) 的 YAML 配置：

| handlist | 原始 YAML |
|---|---|
| `home/major.handlist` | `page/home/major.yml` |
| `home/sides.handlist` | `page/home/sides.yml` |
| `legacy/buttons.handlist` | `page/legacy/buttons.yml` |
| `public/sides.handlist` | `page/public/sides.yml` |
| `tool/major.handlist` | `page/tool/major.yml` |
| `tool/category.handlist` | `page/tool/category.yml` |
| `tool/sides.handlist` | `page/tool/sides.yml` |
| `tool/tools.handlist` | `page/tool/tools.yml` |

改写时做了这些调整：

- 折叠块 `>` 改为字面量 `|`（HTML 渲染不受影响，但换行会被保留）。
- 缩进序列统一增加 2 格缩进。
- 少量“键后下一行写标量”的 YAML 写法（`burning:` 后换行）改为同行标量，因为 handlist 的下一行值必须是映射或序列。
- 引号数字键 `"10":` 改为裸键 `10:`（仍等价于字符串键 `"10"`）。
- 注释保留在示例中；canonical fixture 不保留注释。

[`examples/canonical/`](examples/canonical/) 是规范序列化输出。Rust 与 TypeScript 测试都会校验：解析示例得到的 canonical 文本必须与 fixture 逐字节一致，并且两端共用同一份 fixture。重新生成：

```bash
cd rust
cargo run --example canonicalize
```

## 规范与取舍

完整规范见 [`spec/handlist-spec.md`](spec/handlist-spec.md)。规范包含：

- 文件格式、编码、缩进、注释规则；
- 完整 EBNF 与节点语义；
- 类型推断、安全整数范围、空值/空集合规则；
- canonical 序列化规则；
- 错误报告约定；
- 与 YAML 的功能对照表及移除原因。

几个关键取舍：

- **严格 YAML 1.2 子集**：删除的语法包括锚点/别名/merge、标签、`>` 折叠块、chomping 指示符、单引号字符串、多行 plain/quoted 标量、非空 flow mapping、缩进序列、任意缩进、隐式 null、重复键、YAML 1.1 布尔、十六进制/八进制/指数/日期/sexagesimal 等。
- **键永远是字符串**：避免 `10:`、`true:`、`no:` 在不同 schema 下变成不同类型；映射顺序必须保留。
- **映射键唯一**：重复键直接报错，不做 last-wins。
- **块文本固定语义**：只允许 `|`，不支持 `>` 和 `|-`/`|+`；内容基准缩进固定为父层 +2，末尾不带换行。
- **没有 Unicode/十六进制转义**：只支持 `\"`、`\\`、`\n`、`\t`。这样转义表极小、非 ASCII 文本直接以 UTF-8 书写、可读性和 diff 更稳定，也避免了 Rust 风格 `\u{...}` 与 YAML 的 `\uXXXX` 之间的歧义。代价是其他控制字符无法在字符串中表示——序列化器会报错而不是输出不可读的转义。这个理由也记录在规范第 9.4 节。

## 测试覆盖

- Rust（`--all-features`）：29 个解析/序列化测试、3 个 span 测试、17 个 serde 测试、12 个 CLI 测试、12 个格式化测试、8 个 JSON Schema 测试、1 个示例 + canonical fixture 测试、1 个文档测试。
- TypeScript：26 个解析/序列化/示例/YAML 子集测试，覆盖有序 Map、`__proto__` 键防护、循环引用、数字格式化与错误位置。
- 两端对 8 个示例的 canonical 输出逐字节一致。

## 状态

handlist 1.0，稳定。扩展名 `.handlist`，单文档、单根节点。后续版本应保持：已有合法文档继续合法、已有值不变、canonical 输出可读且可往返、accepted language 仍是 YAML 1.2 的语法子集。
