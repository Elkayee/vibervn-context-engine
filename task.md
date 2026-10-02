# Công việc chuẩn hoá context và thử nghiệm Jev

Ngày cập nhật: 02/10/2026. Repo sở hữu: `C:\Tools\vibervn-context-engine`. Kế hoạch: [plan.md](plan.md).

Người dùng đã yêu cầu triển khai P0 và cho phép Codex tự code sau khi AGY không chạy được. Code/test phần engine đã hoàn tất; CTX-01 đạt khảo sát baseline/source/runtime. Còn 21 mục mở: 6 gate P0 và 15 mục P1–P3; tiến độ cụ thể ghi bên dưới. Không nhập task sửa ChatCmd vào danh sách này.

## DOC — Chuyển tài liệu về đúng repo

- [x] **DOC-01 — Xác định đúng repo sở hữu.** Người dùng xác nhận plan/task phải nằm trong `C:\Tools\vibervn-context-engine`. Verify: đọc được repo qua ChatCMD; root chưa có hai tài liệu, Git baseline sạch.
- [x] **DOC-02 — Kiểm tra hướng dẫn và nội dung nguồn.** Đã kiểm tra root và `.codex/rules/` của repo Context Engine, đọc phần bổ sung Jev trong plan/task ChatCmd và kiểm tra hash trước khi chuyển. Verify: không tìm thấy hướng dẫn root tại engine; phần cũ của hai tài liệu ChatCmd khớp hash trước lần thêm nhầm. Phụ thuộc DOC-01.
- [x] **DOC-03 — Chuyển và kiểm chứng tài liệu.** Tạo plan/task tại root Context Engine, chỉnh đúng phạm vi và chỉ gỡ phần Jev đã thêm nhầm ở ChatCmd. Verify: đọc lại file, đủ 25 ID duy nhất và 22 mục triển khai còn mở; UTF-8/CRLF/link hợp lệ; kiểm tra whitespace; hai tài liệu ChatCmd trở về đúng hash gốc, không thay đổi code hoặc dữ liệu khác. Phụ thuộc DOC-02. Không tự commit/push.

## P0 — Chuẩn hoá trước, Jev tắt

Yêu cầu notebook đã chốt cho CTX-02/03/04/05/07: chỉ lấy `source` của cell `code` trong `.ipynb`; giữ nội dung/thứ tự, path, cell ID/vị trí, dòng trong cell và hash code. Loại Markdown/raw/output/ảnh/base64/execution data trước index/rerank/context; không chạy notebook. Fixture chuỗi/danh sách, Unicode, rỗng và JSON lỗi đã đạt. Checkbox tổng thể còn mở khi gate consumer/provider/runtime chưa kiểm chứng.

- [x] **CTX-01 — Rà luồng thật và chạy baseline.** Đã đối chiếu parse/index/query/rerank/MCP, settings/launcher và routing AGY; runtime PID 11992 có HTTP/MCP đạt, 19 repo, 0 worker hoạt động. Baseline: 659 pass, 2 lỗi cấu hình sẵn có, 6 ignored. AGY không dispatch được; Codex tiếp tục theo chỉ dẫn người dùng. Verify: lệnh/hash/kết quả ở [plan.md](plan.md); runtime vẫn là binary cũ.
- [ ] **CTX-02 — Chốt contract tối thiểu.** Đối chiếu cấu trúc sẵn có trước khi tạo `ContextPackV1`; định nghĩa nguồn, phiên bản, trạng thái, ngân sách và tương thích adapter. Verify: fixture đúng/sai được phân biệt; consumer hiện có không bị phá. Phụ thuộc CTX-01.
- [ ] **CTX-03 — Giữ nguồn và độ mới.** Bảo toàn đường dẫn/dòng/hash/phiên bản, phân biệt nguồn hiện hành với index cũ; không thay bằng chứng bằng tóm tắt. Verify: file sửa/xoá bị đánh dấu đúng; đọc lại kiểm tra phiên bản. Phụ thuộc CTX-02.
- [ ] **CTX-04 — Gộp trùng và tách chẩn đoán.** Không gửi toàn bộ pre-rerank cùng post-rerank; kiểm tra text/structured output. Verify: bằng chứng trùng chỉ gửi một bản cần thiết, chẩn đoán vẫn truy vết ngoài prompt chính. Phụ thuộc CTX-02.
- [ ] **CTX-05 — Giới hạn cứng độc lập Jev.** Kiểm tra token và ký tự/byte áp dụng tại đầu ra tool, trước Jev và trước LLM chính trên toàn payload đã serialize. Verify: payload lớn được chia/lược có thông báo trước khi gửi; không retry vô hạn hoặc mất âm thầm phần bắt buộc. Ngưỡng theo adapter/provider đã xác minh. Phụ thuộc CTX-02.
- [ ] **CTX-06 — Giữ instructions và trạng thái.** Harness quản lý yêu cầu/instructions/quyền đúng phạm vi; dữ liệu truy xuất không trở thành lệnh. Verify: phân biệt warming/rỗng thật/một phần/lỗi; instructions giả trong log không có quyền cao hơn; phần bắt buộc quá ngân sách trả trạng thái cần chia nhỏ. Phụ thuộc CTX-02.
- [ ] **CTX-07 — Khoá regression P0.** Chạy test trước/sau bằng lệnh repo; kiểm tra tính nhất quán lớp chuẩn hoá và các consumer. Verify: test mới tái hiện trước sửa/pass sau sửa, không lỗi mới trong phạm vi, Jev vẫn tắt và 0 request ngoài. Phụ thuộc CTX-03/04/05/06.

Thứ tự: CTX-01 → CTX-02 → CTX-03/04/05/06 → CTX-07. Người dùng đã xác nhận yêu cầu triển khai P0 riêng; không thay gate tổng thể bằng kết quả unit test.

| Mục | Phần engine đã triển khai/kiểm thử | Gate còn mở |
| --- | --- | --- |
| CTX-02 | QueryResult/EvidenceSource/ContextBudget; source schema, fixture hợp lệ/lỗi/rỗng. | Task/turn/quyền và consumer ngoài repo; smoke release mới. |
| CTX-03 | Hash tại parse, cell ID/dòng/hash; origin/freshness/index-only; chặn notebook nguồn đổi/index cũ. | Đọc phục hồi qua consumer/runtime mới; xem ACT-03. |
| CTX-04 | Thứ tự cố định, gộp trùng, không gộp qua cell; diagnostics ngoài final evidence. | Host có nhân đôi text/structured output hoặc gửi diagnostics vào prompt hay không. |
| CTX-05 | Byte cap 48.000 cho serialized final evidence/MCP/body LLM owned; UTF-8/escaping/truncation/requires_split đã test. | Tokenizer/provider threshold, toàn request harness ngoài repo; token_limit_verified=false. |
| CTX-06 | Warming/partial/error/empty, cảnh báo nguồn và requires_split trong engine. | Quyền/instructions do harness sở hữu và kiểm thử lệnh giả ở consumer. |
| CTX-07 | 664 library + 3 context_p0 + 5 notebook_code_only = 672 pass; 6 ignored, 2 baseline skip; rustfmt/check/diff đạt. Mock loopback, Jev tắt. | Kích hoạt và smoke runtime mới; đóng gate CTX-02–06 còn lại. |

Artifact release `target\release\deps\context_engine_rs.exe` đã biên dịch, CLI `--help` đạt. Cargo không copy được vào EXE đang chạy vì Windows lock. Script sao lưu/dừng/copy/khởi động bị duyệt tự động từ chối (`blocked by policy`) trước thực thi; runtime/settings không đổi, chưa reindex workspace thật. Hash/giới hạn kiểm chứng ở [plan.md](plan.md). Không commit/push.

## P1 — Xác minh Jev và chạy shadow

- [ ] **JEV-01 — Xác minh sản phẩm/API.** Xác nhận đúng Jev, nhà cung cấp và nguồn chính chủ về endpoint, SDK/API, đầu ra, batching, model version, rate limit, giá và chính sách dữ liệu. Verify: ghi nguồn/ngày kiểm tra; không cấu hình từ suy đoán `jev.ai` hoặc đặc tính chưa xác minh trong trao đổi trước.
- [ ] **JEV-02 — Quyền gửi dữ liệu và ngân sách.** Chốt workspace/loại dữ liệu/nhà cung cấp được phép, nơi giữ key và bộ lọc secret; không gửi toàn transcript. Verify: off hoặc workspace chưa cho phép tạo 0 request, kể cả shadow; request/log không lộ secret; có ngân sách. Phụ thuộc JEV-01.
- [ ] **JEV-03 — Tập tác vụ và bằng chứng chuẩn.** Chọn ca tìm symbol, lỗi Send, build, cấu hình và nhiều file; người review xác nhận bằng chứng bắt buộc; tách tập hiệu chỉnh/đánh giá. Verify: mỗi ca có yêu cầu, source version, tiêu chí thành công và bằng chứng cần giữ.
- [ ] **JEV-04 — Client tối thiểu bằng mock.** Tái sử dụng interface rerank phù hợp, không dựng framework tổng quát; giữ retrieval score và đánh giá Jev riêng; timeout/request cap/validation/fallback baseline có giới hạn. Verify: mock timeout/quota/thiếu key/sai schema không làm mất context; không mặc định nối hai reranker. Phụ thuộc JEV-01 và CTX-07.
- [ ] **JEV-05 — Câu hỏi đánh giá có phiên bản.** Hỏi các đánh giá hẹp phục vụ tác vụ, xác minh batching thật; không coi confidence là bảo đảm đúng hoặc đặt ngưỡng lọc tuỳ ý. Verify: câu hỏi/model/config/evidence truy vết được. Phụ thuộc JEV-01.
- [ ] **JEV-06 — Shadow có kiểm soát.** Chỉ gọi dịch vụ sau khi dữ liệu được phép; ghi đánh giá, chi phí/token, độ trễ và lỗi nhưng không đổi context chính. Verify: context shadow bằng nhánh chuẩn hoá không Jev, số liệu đo thật, fallback pass. Phụ thuộc JEV-02/03/04/05.

## P2 — Một workspace, xếp hạng trước rồi mới lược bỏ

- [ ] **ACT-01 — Workspace, mode và rollback.** Chọn rõ workspace thử nghiệm, giữ off/shadow/active với off mặc định. Verify: đổi off dừng mọi request Jev, không ảnh hưởng workspace khác; có cấu hình rollback. Phụ thuộc JEV-06.
- [ ] **ACT-02 — Hiệu chỉnh chính sách giữ/bỏ.** Chọn ngưỡng trên tập hiệu chỉnh rồi khoá trước đánh giá; rerank trước, không loại bằng chứng bắt buộc; không coi thiếu bằng chứng là không liên quan. Verify: ngoại lệ ở cuối đoạn/ít từ khoá vẫn xử lý đúng; phần chưa đưa vào được thông báo và có đường đọc lại. Phụ thuộc ACT-01.
- [ ] **ACT-03 — Đọc phục hồi an toàn.** Tái sử dụng tham chiếu evidence/khoảng dòng nếu đủ; kiểm tra quyền/workspace/phiên bản và ngân sách khi đọc. Verify: path traversal, tham chiếu sai workspace hoặc nguồn đổi không trả nhầm dữ liệu. Phụ thuộc CTX-03 và ACT-01.
- [ ] **ACT-04 — Cache đúng phạm vi khi có cache.** Không thêm cache nếu chưa cần; cache hiện có phải xét task/instructions/workspace/evidence hash/model/bộ câu hỏi/cấu hình. Verify: đổi đầu vào ảnh hưởng quyết định không dùng lại kết quả cũ; nếu không có cache, ghi không áp dụng cùng bằng chứng. Phụ thuộc ACT-01.
- [ ] **ACT-05 — Test đối kháng và rollback.** Bao phủ Jev lỗi, warming, nguồn cũ, log có lệnh giả, ngoại lệ, payload lớn, đọc phục hồi và đổi mode. Verify: test pass; rollback khôi phục baseline có giới hạn, không mất nguồn/instructions/cảnh báo và dừng request Jev. Phụ thuộc ACT-01/02/03/04.

## P3 — Đánh giá trước khi mở rộng

- [ ] **EVAL-01 — Chốt tiêu chí trước khi đo.** Ghi tiêu chí chất lượng/bằng chứng, ngân sách chi phí/độ trễ và độ đầy đủ tập đánh giá. Verify: có tiêu chí được xác nhận trước khi xem kết quả; tập đánh giá chưa dùng để hiệu chỉnh. Phụ thuộc JEV-03.
- [ ] **EVAL-02 — So sánh A/B/C.** A: baseline; B: chuẩn hoá/gộp trùng/giới hạn không Jev; C: B cộng Jev. Verify: cùng tập tác vụ, ghi model/source/prompt/config/cache; không gửi payload vô hạn ra provider để đo A, không bịa số liệu. Phụ thuộc ACT-05 và EVAL-01.
- [ ] **EVAL-03 — Lợi ích toàn hệ thống.** Đo task success, evidence recall, tổng token/chi phí Jev + LLM chính, p50/p95 latency, số lần đọc phục hồi và lỗi/fallback. Verify: tách lợi ích B so với A và C so với B, ghi hạn chế/độ bất định. Phụ thuộc EVAL-02.
- [ ] **EVAL-04 — Giữ hoặc đề xuất mở rộng.** Chỉ đề xuất mở rộng khi đạt điều kiện đã chốt và C có lợi ích thêm so với B; nếu không giữ B, Jev off. Verify: kết luận theo số liệu, có rollback và trạng thái plan/task đúng bằng chứng; mở rộng cần yêu cầu riêng. Phụ thuộc EVAL-02/03.

## Quy tắc thực hiện và đánh dấu hoàn tất

Chỉ sửa dòng truy về yêu cầu; đọc trước khi patch, giữ style và thay đổi sẵn có. Không dọn code ngoài phạm vi hoặc xoá việc cũ. Codex review/điều phối; AGY ưu tiên triển khai, riêng P0 này Codex tự code theo ngoại lệ người dùng đã cho phép. Mỗi mục chỉ chuyển [x] khi có file/diff, kiểm thử hoặc quyết định xác nhận. Mock không chứng minh API thật hoặc runtime mới đã chạy; DOC hoàn tất không đồng nghĩa P0–P3 hoàn tất. Không tự commit/push, đổi transport/orchestrator, cài Jev hoặc bật mọi workspace.

Người dùng đã yêu cầu commit, merge vào nhánh chính và xoá nhánh triển khai sau khi merge. Quyền Git này thay thế hạn chế không tự commit/push của lượt P0 trước; không đóng các gate runtime, tokenizer/provider hoặc consumer còn mở.
