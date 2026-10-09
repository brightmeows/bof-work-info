# AGENTS.md

manbow BMS 活动数据仓库：fetcher 抓取、GitHub Actions 周更与时更、数据以 TOML 存档。仓库结构、工具用法与数据格式见 README.md 与 fetcher/README.md，此处只写代码与工作流的约定。

## events/*.toml 是生成物

数据文件由 fetcher 全量覆盖，手动修改会在下次运行被冲掉；要改数据请改抓取逻辑或以页面源为准。

fetcher 的输出必须保持确定性：相同输入产出字节相同的文件，CI 以 git diff 判定是否建 PR。改动解析或序列化后，跑一次真实抓取并确认 diff 只含真实数据变化，才算完成。

## 解析器约定

manbow 页面解析全部采用断言式表头/字段匹配：列名或字段行落在已知集合内才继续，否则报错降级；写新解析器沿用该模式，并先抓一份真实页面样本再动手。调试用 `--stdin` 喂 URL 可打印单页解析结果。HTML 解码走 meta charset 嗅探加严格解码链，站点实际编码为 Windows-31J（WHATWG `shift_jis`）。

## 活动生命周期维护

进行中活动的 event id（当前 152、153）硬编码在两处：update-hourly.yml 的 HOURLY_EVENTS 与 update-events.yml 的 `--details` 循环。活动落幕评分冻结后，hourly job 退化为快速空转，届时手动调整这两处并视情况增删新活动。

## 提交门槛

- `cargo clippy --workspace --all-targets` 零警告（pedantic deny 配置在根 Cargo.toml 的 workspace lints）
- `cargo fmt --all --check` 干净
- Conventional Commits：`type(scope): subject`，提交信息用简体中文（type 与 scope 保留英文）

## 诊断注意

本机若启用透明代理（DNS 解析出 198.18.x fake-ip 网段），curl 与 python 经代理出口访问 manbow 会拿到反爬确认页，fetcher 的请求路径则正常。排查抓取问题以 fetcher 实际抓到的字节为准，页面结构样本一律取自 fetcher 成功的响应，确认页样本不能用于推断页面结构。对同一 URL 连续压测会触发站点反爬，抓取代码保持请求间隔（`--delay-ms`）。
