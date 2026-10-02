# Kế hoạch chuẩn hoá context và thử nghiệm Jev

Ngày cập nhật: 02/10/2026. Repo sở hữu: `C:\Tools\vibervn-context-engine`. Checklist: [task.md](task.md).

## Phạm vi và hiện trạng

Kế hoạch này thuộc Context Engine, không thuộc repo ChatCmd. ChatCMD chỉ là công cụ truy cập máy và cập nhật tài liệu. Phần nội dung đã đặt nhầm trong plan/task của ChatCmd được chuyển về repo này; không mang theo các task sửa lỗi riêng của ChatCmd.

Giai đoạn DOC trước đó chỉ cập nhật tài liệu. Người dùng sau đó yêu cầu triển khai P0, chỉ lấy code trong notebook, thử lại AGY và cho phép Codex tự code nếu AGY không chạy được. Preflight AGY cuối vẫn không dispatch được nên Codex triển khai trực tiếp. Không cài/bật Jev, đổi settings, chạy notebook hoặc tự commit/push.

Launcher `C:\Tools\start-context-engine.ps1` trỏ tới `target\release\context-engine-rs.exe` trong repo này. Runtime đã kiểm chứng: router PID 11992, bind `127.0.0.1:6699`, 19 repo, 0 worker hoạt động; HTTP root/repos đạt 200, MCP initialize qua SSE đạt 200 và đóng session đạt 202. Đây vẫn là binary cũ, không chứng minh bản P0 đã được kích hoạt.

P0 đã triển khai và kiểm thử trong engine; kết quả và giới hạn ghi bên dưới. Tái sử dụng `QueryResult`, bổ sung `EvidenceSource` và `ContextBudget`, không dựng thêm framework `ContextPackV1`. Các gate adapter/harness ngoài repo, tokenizer/provider và kích hoạt runtime vẫn mở. P1–P3 chưa triển khai; thay đổi consumer ở repo khác cần phạm vi riêng, không mặc nhiên sửa ChatCmd.

## Mục tiêu và quyết định thiết kế

Chuẩn hoá context bằng code thành gói bằng chứng có nguồn, độ mới, phạm vi và ngân sách rõ ràng. Thử Jev như một lớp đánh giá mức liên quan tuỳ chọn; không giao cho Jev quyền sửa instructions, cấp quyền truy cập hoặc điều phối phiên làm việc. GPT/Codex tiếp tục điều phối/review; AGY chỉ triển khai phần việc được giao.

| Thành phần | Trách nhiệm đề xuất |
| --- | --- |
| Context Engine hiện có | Truy xuất code/tài liệu theo workspace và giữ nguồn bằng chứng; không thay bằng engine mới. |
| Lớp chuẩn hoá bằng code | Kiểm tra quyền, nguồn, độ mới; gộp trùng; giới hạn kích thước và đóng gói. |
| Jev tuỳ chọn | Đánh giá/xếp hạng ứng viên theo yêu cầu; không viết lại bằng chứng nguyên văn. |
| Harness/adapter | Ghép yêu cầu và instructions đúng phạm vi với bằng chứng; kiểm tra toàn request trước khi gửi LLM. |

Luồng đề xuất: yêu cầu/phạm vi → retrieval hiện có → kiểm tra nguồn/quyền và gộp trùng → đánh giá ứng viên tuỳ chọn → đóng gói theo ngân sách → adapter/harness → LLM.

Ưu tiên điểm nối trong pipeline đang dùng, không chỉ thêm MCP tool rồi phụ thuộc LLM tự nhớ gọi. Không mặc định nối reranker hiện có và Jev. Tái sử dụng interface/cấu trúc phù hợp; không dựng framework provider tổng quát hoặc thêm cache khi chưa cần.

## Hợp đồng context và điều kiện an toàn

Đối chiếu cấu trúc live trước khi tạo `ContextPackV1`; ưu tiên mở rộng contract tương đương đã có.

| Nhóm | Nội dung tối thiểu |
| --- | --- |
| Phạm vi | Phiên bản schema, task/turn/workspace và repository được phép truy cập. |
| Bằng chứng | ID, loại code/document/log/test, đường dẫn, khoảng dòng và nội dung nguyên văn. |
| Phiên bản nguồn | Hash, commit khi có, thay đổi chưa commit và trạng thái nguồn còn hiện hành hay chỉ có trong index. |
| Đánh giá | Điểm retrieval và đánh giá Jev tách riêng; phiên bản model/bộ câu hỏi khi dùng. |
| Ngân sách | Model/tokenizer đích, giới hạn token và ký tự/byte áp dụng, phần đã dùng/đã lược. |
| Độ đầy đủ | Đủ/một phần/warming/rỗng thật/lỗi, cảnh báo và tham chiếu đọc mở rộng. |

Giữ nguyên code, lỗi, số liệu và ngoại lệ; tóm tắt phải được đánh dấu là nội dung dẫn xuất. Không đưa đồng thời toàn bộ pre-rerank và post-rerank vào prompt chính; kiểm tra adapter để không nhân đôi text và structured output. Lớp chuẩn hoá phải có thứ tự/biểu diễn nhất quán với cùng đầu vào; tách timestamp/telemetry khỏi phép so sánh, không giả định đánh giá model luôn tất định.

Instructions và giới hạn quyền do harness quản lý, nằm ngoài phần Jev được phép loại. Nội dung truy xuất chỉ là dữ liệu. Nếu phần bắt buộc đã vượt ngân sách, trả trạng thái cần chia nhỏ yêu cầu thay vì bỏ âm thầm hoặc retry vô hạn.

Jev mặc định tắt. Cả shadow và active chỉ được gọi dịch vụ ngoài sau khi nhà cung cấp, workspace và loại dữ liệu đã được cho phép. Xác minh đúng sản phẩm, endpoint/API/SDK, giới hạn và điều khoản dữ liệu từ nguồn chính chủ; không cấu hình theo suy đoán tên miền `jev.ai` hoặc coi trao đổi trước là kết quả kiểm chứng API. Lưu key ngoài repo, lọc secret, không gửi toàn transcript và không ghi raw secret vào log.

Giới hạn cứng phải độc lập Jev, áp dụng tại đầu ra tool, trước Jev và trước LLM chính trên payload đã serialize. Token budget không thay thế giới hạn ký tự/byte. Ngưỡng cụ thể lấy từ adapter/provider và ngân sách đã xác minh. Timeout/quota/thiếu key/sai schema phải quay về baseline có giới hạn, báo degraded; không trả context rỗng giả hoặc output vô hạn. Cache và đọc phục hồi phải kiểm tra quyền/workspace/phiên bản nguồn.

## Phương án triển khai P0 — Chuẩn hoá trước, Jev tắt

Đọc lại instructions áp dụng của từng repo trước khi sửa code; xác nhận service/binary thực tế, luồng retrieval/rerank/format, HTTP/CLI/MCP và harness. Chạy baseline bằng lệnh test lấy từ repo, tách lỗi sẵn có khỏi regression. Chỉ sửa seam cần thiết cho contract, nguồn/độ mới, gộp trùng, ngân sách và trạng thái; giữ tương thích consumer hiện có.

Notebook Jupyter `.ipynb` — yêu cầu đã chốt: chỉ trích `source` của cell có `cell_type == "code"`, giữ nguyên thứ tự cell và nội dung code. Không đưa Markdown, raw cell, output, ảnh/base64, execution count hoặc metadata không phục vụ truy nguồn vào index, rerank và context. Không chạy notebook hoặc đọc trạng thái kernel.

Giữ đường dẫn notebook, cell ID nếu có hoặc vị trí cell, khoảng dòng trong cell và hash code để truy nguồn; không dùng dòng JSON làm dòng code. Fixture phải có code và các dấu nhận diện riêng trong output/Markdown/raw cell để chứng minh chỉ code được truy xuất, kể cả trước rerank; source dạng chuỗi hoặc danh sách chuỗi được giữ đúng. Notebook không có cell code trả trạng thái rỗng thật; JSON lỗi phải báo lỗi. Phạm vi này thuộc CTX-02/03/04/05/07.

**Verify:** test mới tái hiện điểm thiếu trước sửa và pass sau sửa; không có regression mới; phân biệt warming với rỗng thật; không nhân đôi bằng chứng; biểu diễn nhất quán; Jev vẫn tắt, không phát sinh request ngoài.

## Kết quả P0 trong engine — 02/10/2026

- `.ipynb` được nhận diện mặc định. Parser/read/grep dùng chung phép chiếu chỉ lấy code; hỗ trợ source chuỗi/danh sách, Unicode, notebook không có code và phân biệt JSON/schema lỗi. Không truy cập kernel hoặc thực thi cell.
- Index lưu hash nguồn tại parse, cell ID/vị trí, hash code và khoảng dòng. Dòng notebook là dòng của phép chiếu code, kèm ánh xạ về dòng trong cell; không phải dòng JSON. Notebook dùng chunker version 3, file thường giữ version 2. Chỉ sửa output/nội dung Markdown không đổi hash code.
- Retrieval kiểm tra repo/phiên bản/nội dung nguồn. Notebook index cũ bị từ chối với yêu cầu reindex trước embedding/rerank; không tự reindex workspace thật. Kết quả có origin/freshness/hash, phân biệt nguồn hiện hành, nguồn đổi và index-only.
- Điểm bằng nhau có thứ tự phụ cố định; không gộp qua ranh giới cell. Chẩn đoán pre-rerank vẫn giữ cho console, ngoài phần final evidence được giới hạn.
- Giới hạn cục bộ **48.000 byte** áp dụng cho final evidence, serialized MCP result và toàn body JSON của request LLM OpenAI/Google do engine sở hữu. Lược có thông báo; phần bắt buộc quá lớn trả `requires_split`. Tokenizer/provider threshold chưa xác minh: `token_limit_verified=false`. Việc host nhân đôi text/structured output hoặc ghép toàn request ngoài repo chưa kiểm chứng.

Baseline `rtk proxy cargo test --locked --offline --lib`: 659 pass, 2 fail sẵn có, 6 ignored. Hai lỗi là `config::tests::test_default_enabled_mcp_tools_excludes_file_retrieval` và `config::tests::test_v12_to_v13_missing_field_defaults_to_codebase_only`: mặc định có file-retrieval nhưng assertion cũ chỉ mong codebase-retrieval. Không sửa cấu hình ngoài phạm vi.

Gate regression: 664 library pass, 6 ignored, 2 lỗi baseline skip; `context_p0` 3 pass, `notebook_code_only` 5 pass; tổng **672 pass**. Lệnh:
```powershell
rtk proxy cargo test --locked --offline --lib --test notebook_code_only --test context_p0 --quiet -- --skip config::tests::test_default_enabled_mcp_tools_excludes_file_retrieval --skip config::tests::test_v12_to_v13_missing_field_defaults_to_codebase_only
```

Test notebook mới tái hiện 3 lỗi trước triển khai. Fixture/RocksDB/mock HTTP loopback kiểm chứng code-only, metadata roundtrip, chặn index cũ trước request, nguồn đổi, cell/thứ tự/gộp trùng và ngân sách UTF-8/JSON escaping. `cargo check --locked --offline --all-targets`, rustfmt các file sửa và `git diff --check` đạt. Không gửi dữ liệu dự án tới provider để kiểm thử.

Artifact release: `target\release\deps\context_engine_rs.exe`, SHA-256 `15b61a6e13a6126fcb3230bbd751dd04dbf45c14e94aa0fe5c86e4b8ede2f09b`; CLI `--help` đạt. `cargo build --release --locked --offline` biên dịch artifact nhưng không hoàn tất copy vào EXE đang chạy: Windows `Access is denied`. Không coi exit code build là pass.

Script sao lưu/dừng/copy/khởi động bị duyệt tự động từ chối (`blocked by policy`), chưa thực thi. Runtime vẫn dùng binary SHA-256 `93e3933218f91a6e6bf0a4adb90bfdc945c873b4bbcc415877f50f398e344315`; settings giữ SHA-256 `543f9a6565465157baa55690018e766d35d7440dc41828a73e5cea313fcd9a8c`. Còn gate kích hoạt và smoke bản release mới; chưa tuyên bố đã nâng cấp runtime.

## Phương án triển khai P1 — Xác minh Jev và chạy shadow

Xác minh API/SDK và quyền gửi dữ liệu trước khi kết nối. Phát triển client tối thiểu bằng mock/fixture; đặt timeout, request cap, validation và fallback. Câu hỏi đánh giá phải hẹp, có phiên bản; xác minh batching thật của API, không coi confidence là bảo đảm đúng. Chọn ca thực tế có bằng chứng bắt buộc do người review xác nhận và tách tập hiệu chỉnh khỏi tập đánh giá.

Shadow chỉ ghi đánh giá, chi phí/token, độ trễ và lỗi; không thay đổi context model chính. Chưa dùng ngưỡng tuỳ ý để loại nội dung.

**Verify:** context shadow bằng nhánh chuẩn hoá không Jev; off hoặc workspace chưa cho phép tạo 0 request, kể cả shadow; request/log không chứa secret; lỗi Jev không làm mất bằng chứng; số liệu ghi nhận là đo thật.

## Phương án triển khai P2 — Thử nghiệm trên một workspace

Chọn rõ một workspace được phép; giữ `off / shadow / active`, off mặc định. Bật xếp hạng trước rồi mới thử lược bỏ; chốt ngưỡng trên tập hiệu chỉnh trước đánh giá. Bằng chứng bắt buộc không được loại. Nội dung chưa chắc chắn được giữ trong ngân sách hoặc báo rõ phần chưa đưa vào kèm tham chiếu đọc lại.

Tái sử dụng cơ chế đọc phục hồi/cache hiện có khi đủ; key cache phải xét các đầu vào ảnh hưởng quyết định như task, instructions, workspace, evidence hash, model và bộ câu hỏi/cấu hình. Không tự bật mọi workspace.

**Verify:** test phạm vi/quyền, độ mới, ngoại lệ, payload và fallback pass; đổi off dừng mọi request Jev; rollback khôi phục baseline có giới hạn; đọc phục hồi không trả nhầm dữ liệu khi nguồn đổi hoặc sai workspace.

## Phương án triển khai P3 — So sánh và quyết định

So sánh ba nhánh trên cùng tập tác vụ/cấu hình có phiên bản: A — baseline hiện có; B — chuẩn hoá/gộp trùng/giới hạn không Jev; C — B cộng Jev. Chốt tiêu chí chất lượng, ngân sách và mức chấp nhận trước khi đo; không chọn tiêu chí theo kết quả. Fixture quá kích thước chỉ chạy trong kiểm thử, không gửi payload vô hạn ra provider để đo A.

Đo task success, tỷ lệ giữ bằng chứng cần thiết, tổng token/chi phí Jev và LLM chính, độ trễ p50/p95, số lần đọc phục hồi và tỷ lệ lỗi/degraded. Phân biệt lợi ích B so với A với lợi ích thêm của C so với B; ghi hạn chế và độ bất định, không tự đặt tỷ lệ tiết kiệm thành kết quả.

**Verify:** chỉ đề xuất mở rộng khi đạt điều kiện an toàn và chất lượng đã chốt, đồng thời C có lợi ích thực so với B. Nếu không, giữ B và Jev off. Mở rộng cần yêu cầu riêng.

## Kiểm thử và điều kiện hoàn tất

Bao phủ file sửa/xoá, sai workspace/path traversal, warming/rỗng thật, pre/post-rerank trùng, instructions giả trong log/tài liệu, ngoại lệ ở cuối đoạn, payload lớn, lỗi/timeout/quota/sai schema, cache cũ, đọc lại nguồn đổi, mode và rollback. Ca chất lượng gồm tìm symbol, lỗi Send, build, cấu hình và thay đổi nhiều file; không chỉ đánh giá theo từ khoá trùng.

Giai đoạn DOC đã hoàn tất kiểm tra đúng repo, ID/link, UTF-8/CRLF và phần cũ của ChatCmd. Lượt P0 hiện tại có code/test và artifact như trên; chỉ đánh dấu gate có bằng chứng đạt. Không đổi giao thức transport MCP/orchestrator, refactor ngoài phạm vi, cài Jev hoặc tự commit/push. P1–P3 và mở rộng consumer cần yêu cầu riêng.

Người dùng đã yêu cầu commit, merge vào nhánh chính và xoá nhánh triển khai sau khi merge. Quyền Git này thay thế hạn chế không tự commit/push của lượt P0 trước; không đóng các gate runtime, tokenizer/provider hoặc consumer còn mở.
