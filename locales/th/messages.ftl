# ทรัพยากรการแปลภาษาสำหรับบรรทัดคำสั่งของ Netsuke

runner.io.dyndep.retention = ไม่สามารถใช้การเก็บรักษา dyndep ที่สร้างขึ้นใต้ { $path } ได้
cli.about = Netsuke คอมไพล์ไฟล์รายการ YAML + Jinja ให้เป็นแผนการสร้างของ Ninja
cli.long_about = Netsuke แปลงไฟล์รายการ YAML + Jinja ให้เป็นกราฟ Ninja ที่สร้างซ้ำได้ แล้วเรียกใช้ Ninja ด้วยค่าเริ่มต้นที่ปลอดภัย
cli.usage = { $usage }

# ข้อความช่วยเหลือของตัวเลือกทั่วไป
cli.flag.file.help = เส้นทางของไฟล์รายการ Netsuke ที่จะใช้
cli.flag.directory.help = ทำงานเสมือนว่าเริ่มต้นในไดเรกทอรีนี้
cli.flag.config.help = เส้นทางของไฟล์ตั้งค่า โดยข้ามการค้นหาอัตโนมัติ
cli.flag.jobs.help = กำหนดจำนวนงานสร้างที่ทำงานขนานกัน
cli.flag.verbose.help = เปิดบันทึกวินิจฉัยแบบละเอียดและสรุปเวลาที่ใช้เมื่อเสร็จสิ้น
cli.flag.locale.help = แท็กภาษาสำหรับข้อความบรรทัดคำสั่ง (เช่น en-US, th)
cli.flag.fetch_allow_scheme.help = สกีม URL เพิ่มเติมที่อนุญาตให้ตัวช่วย fetch ใช้
cli.flag.fetch_allow_host.help = ชื่อโฮสต์ที่อนุญาตเมื่อเปิดการปฏิเสธโดยค่าเริ่มต้น
cli.flag.fetch_block_host.help = ชื่อโฮสต์ที่ถูกปิดกั้นเสมอ แม้จะได้รับอนุญาตจากที่อื่น
cli.flag.fetch_default_deny.help = ปฏิเสธโฮสต์ทั้งหมดโดยค่าเริ่มต้น อนุญาตเฉพาะรายการที่ประกาศไว้
cli.flag.trust_project_fetch_policy.help = Allow project configuration to widen fetch-policy grants.
cli.flag.json.help = แสดงผลเป็น JSON ที่เครื่องอ่านได้
cli.flag.no_input.help = ไม่อ่านข้อมูลนำเข้าแบบโต้ตอบเลย
cli.flag.color.help = นโยบายการแสดงผลแบบมีสี (auto, always, never)
cli.flag.emoji.help = นโยบายอิโมจิ (auto, always, never)
cli.flag.progress.help = นโยบายการแสดงความคืบหน้า (auto, always, never)
cli.flag.accessibility.help = นโยบายการแสดงผลที่เข้าถึงได้ (auto, on, off)
cli.flag.default_targets.help = เป้าหมายการสร้างโดยปริยายเมื่อไม่ได้ระบุเป้าหมายใด

# คำอธิบายคำสั่งย่อย
cli.subcommand.build.about = สร้างเป้าหมายที่กำหนดไว้ในไฟล์รายการ (ค่าเริ่มต้น)
cli.subcommand.build.long_about = สร้างเป้าหมายที่ร้องขอ หากไม่ได้ระบุ จะใช้เป้าหมายโดยปริยายของไฟล์รายการ
cli.subcommand.clean.about = ลบสิ่งที่สร้างขึ้นผ่าน Ninja
cli.subcommand.clean.long_about = สร้างไฟล์ Ninja ชั่วคราว จากนั้นเรียกใช้ `ninja -t clean`
cli.subcommand.graph.about = แสดงกราฟการพึ่งพาของการสร้าง รูปแบบเริ่มต้นคือ DOT
cli.subcommand.graph.long_about = ฉายไฟล์รายการ Netsuke ที่แจงแล้วให้เป็นกราฟการสร้างมาตรฐาน แล้วเขียนเป็น Graphviz DOT หรือเขียนเป็นหน้า HTML ที่สมบูรณ์ในตัวเมื่อใช้ `--html` ใช้ `--output <ไฟล์>` เพื่อเขียนลงไฟล์ ส่วน `-` จะเขียนไปยังเอาต์พุตมาตรฐาน
cli.subcommand.generate.about = สร้างไฟล์รายการ Ninja โดยไม่เรียกใช้ Ninja
cli.subcommand.generate.long_about = เขียนไฟล์รายการ Ninja ที่สร้างขึ้นไปยังเอาต์พุตมาตรฐาน หรือไปยังไฟล์ที่เลือกด้วย `--output`
cli.subcommand.help.about = พิมพ์ความช่วยเหลือระดับบนสุด หรือความช่วยเหลือสำหรับหัวข้อที่ระบุชื่อ
cli.subcommand.help.long_about = หากไม่มีหัวข้อ คำสั่งนี้จะเหมือนกับ `--help` ใช้ `help targets` เพื่อพิมพ์แคตตาล็อกเป้าหมายและการดำเนินการสำหรับไฟล์รายการที่เลือก

# Help catalogue headings and markers.
cli.help.actions_heading = การดำเนินการ:
cli.help.targets_heading = เป้าหมาย:
cli.help.targets.about = แสดงรายการเป้าหมายและการดำเนินการในไฟล์รายการที่เลือก
cli.help.default_marker = ค่าเริ่มต้น
cli.help.conditional_marker = มีเงื่อนไข

# ข้อความช่วยเหลือของตัวเลือกในคำสั่งย่อย build
cli.subcommand.build.flag.targets.help = เป้าหมายที่จะสร้าง (หากละไว้ จะใช้ค่าโดยปริยายของไฟล์รายการ)

# ข้อความช่วยเหลือของตัวเลือกในคำสั่งย่อย graph
cli.subcommand.graph.flag.html.help = แสดงกราฟเป็นหน้า HTML ที่สมบูรณ์ในตัวแทนรูปแบบ DOT
cli.subcommand.graph.flag.output.help = เขียนผลลัพธ์กราฟลงไฟล์ ใช้ `-` สำหรับเอาต์พุตมาตรฐาน

# ข้อความช่วยเหลือของตัวเลือกในคำสั่งย่อย generate
cli.subcommand.generate.flag.output.help = เขียนไฟล์รายการ Ninja ที่สร้างขึ้นลงไฟล์แทนเอาต์พุตมาตรฐาน

# ข้อผิดพลาดในการตรวจสอบบรรทัดคำสั่ง
cli.validation.jobs.invalid_number = { $value } ไม่ใช่ตัวเลขที่ถูกต้อง
cli.validation.jobs.out_of_range = จำนวนงานต้องอยู่ระหว่าง { $min } ถึง { $max }
cli.validation.scheme.empty = สกีมต้องไม่ว่างเปล่า
cli.validation.scheme.invalid_start = สกีม “{ $scheme }” ต้องขึ้นต้นด้วยอักษร ASCII
cli.validation.scheme.invalid = สกีมไม่ถูกต้อง: “{ $scheme }”
cli.validation.locale.empty = แท็กภาษาต้องไม่ว่างเปล่า
cli.validation.locale.invalid = แท็กภาษาไม่ถูกต้อง: “{ $locale }”
cli.validation.color.invalid = นโยบายสีไม่ถูกต้อง: “{ $value }” ค่าที่ใช้ได้: auto, always, never
cli.validation.emoji.invalid = นโยบายอิโมจิไม่ถูกต้อง: “{ $value }” ค่าที่ใช้ได้: auto, always, never
cli.validation.progress.invalid = นโยบายความคืบหน้าไม่ถูกต้อง: “{ $value }” ค่าที่ใช้ได้: auto, always, never
cli.validation.accessibility.invalid = นโยบายการเข้าถึงไม่ถูกต้อง: “{ $value }” ค่าที่ใช้ได้: auto, on, off
cli.validation.config.expected_object = ค่าจากบรรทัดคำสั่งควรถูกทำให้เป็นลำดับข้อมูลแบบวัตถุ แต่ได้ { $value }

# ข้อความแสดงข้อผิดพลาดของ Clap
clap-error-missing-argument = ขาดอาร์กิวเมนต์ที่จำเป็น: { $argument }
clap-error-missing-subcommand = ขาดคำสั่งย่อย ตัวเลือกที่ใช้ได้: { $valid_subcommands }
clap-error-unknown-argument = อาร์กิวเมนต์ที่ไม่รู้จัก: { $argument }
clap-error-invalid-value = ค่าของ { $argument } ไม่ถูกต้อง: { $value }
clap-error-invalid-subcommand = คำสั่งย่อยที่ไม่รู้จัก: { $subcommand }
# หมายเหตุ: value-validation ใช้ถ้อยคำต่างจาก invalid-value เพื่อแยกความล้มเหลว
# ของตัวตรวจสอบที่กำหนดเอง (ErrorKind::ValueValidation) ออกจากชนิดที่ไม่ตรงกัน
# (ErrorKind::InvalidValue)
clap-error-value-validation = การตรวจสอบ { $argument } ล้มเหลว: { $value }

# ข้อผิดพลาดและบริบทขณะทำงาน
runner.manifest.not_found = ไม่พบไฟล์รายการ “{ $manifest_name }” ใน { $directory }
runner.manifest.not_found.help = โปรดตรวจสอบว่าไฟล์รายการมีอยู่จริง หรือระบุ `--file` ด้วยเส้นทางที่ถูกต้อง
runner.manifest.path_missing_name = เส้นทางไฟล์รายการ “{ $path }” ไม่มีชื่อไฟล์
cli.file.non_utf8 = เส้นทางไฟล์รายการ “{ $path }” ไม่ใช่ UTF-8 ที่ถูกต้อง
runner.manifest.directory_label = ไดเรกทอรี `{ $directory }`
runner.manifest.current_directory_label = ไดเรกทอรีปัจจุบัน
runner.manifest.default_not_declared = ค่าเริ่มต้นของรายการ '{ $default }' ไม่ได้ระบุการดำเนินการหรือเป้าหมายที่ประกาศไว้
runner.context.network_policy = สร้างนโยบายเครือข่ายไม่สำเร็จ
runner.context.load_manifest = โหลดไฟล์รายการที่ { $path } ไม่สำเร็จ
runner.context.serialise_manifest = ทำให้ไฟล์รายการเป็นลำดับข้อมูลไม่สำเร็จ
runner.context.build_graph = สร้างกราฟจากไฟล์รายการไม่สำเร็จ
runner.context.generate_ninja = สร้างไฟล์รายการ Ninja ไม่สำเร็จ
runner.context.render_graph = แสดงผลลัพธ์กราฟไม่สำเร็จ

runner.io.create_temp_file = สร้างไฟล์ Ninja ชั่วคราวไม่สำเร็จ
runner.io.write_temp_ninja = เขียนไฟล์ Ninja ชั่วคราวไม่สำเร็จ
runner.io.flush_temp_ninja = ล้างบัฟเฟอร์ของไฟล์ Ninja ชั่วคราวไม่สำเร็จ
runner.io.sync_temp_ninja = ประสานข้อมูลไฟล์ Ninja ชั่วคราวไม่สำเร็จ
runner.io.create_parent_dir = สร้างไดเรกทอรีแม่ { $path } ไม่สำเร็จ
runner.io.create_ninja_file = สร้างไฟล์ Ninja ที่ { $path } ไม่สำเร็จ
runner.io.write_ninja_file = เขียนไฟล์ Ninja ที่ { $path } ไม่สำเร็จ
runner.io.flush_ninja_file = ล้างบัฟเฟอร์ของไฟล์ Ninja ที่ { $path } ไม่สำเร็จ
runner.io.sync_ninja_file = ประสานข้อมูลไฟล์ Ninja ที่ { $path } ไม่สำเร็จ
runner.io.open_ambient_dir = เปิดไดเรกทอรีโดยรอบไม่สำเร็จ
cli.directory.non_utf8 = เส้นทางไดเรกทอรีทำงานไม่ใช่ UTF-8 ที่ถูกต้อง ({ $path })
runner.io.no_existing_ancestor = ไม่มีไดเรกทอรีระดับบนที่มีอยู่จริงสำหรับ { $path }
runner.io.derive_relative_path = อนุมานเส้นทางสัมพัทธ์ของ Ninja ไม่สำเร็จ
runner.io.non_utf8_path = ไม่รองรับเส้นทางที่ไม่ใช่ UTF-8 (เส้นทาง: { $path })
runner.io.write_stdout = เขียนไฟล์รายการ Ninja ไปยังเอาต์พุตมาตรฐานไม่สำเร็จ
runner.io.flush_stdout = ล้างบัฟเฟอร์ของเอาต์พุตมาตรฐานไม่สำเร็จ
runner.io.dyndep.create_dir = ไม่สามารถสร้างไดเรกทอรี dyndep { $path } ได้
runner.io.dyndep.read = ไม่สามารถอ่านไฟล์ dyndep ที่สร้างขึ้นที่ { $path } ได้
runner.io.dyndep.write = ไม่สามารถเขียนไฟล์ dyndep ที่สร้างขึ้นไปยัง { $path } ได้
runner.io.dyndep.rename = ไม่สามารถเปลี่ยนชื่อไฟล์ dyndep ที่สร้างขึ้นที่ { $path } ได้
runner.io.dyndep.corrupt = ไฟล์ dyndep ที่สร้างขึ้นที่ { $path } ไม่ตรงกับเนื้อหาที่คาดไว้ ให้ลบเฉพาะไฟล์นี้แล้วลองอีกครั้ง
runner.io.dyndep.temp_collisions = ไม่สามารถสร้างไฟล์ dyndep ชั่วคราวที่ไม่ซ้ำกันสำหรับ { $path } ได้หลังเกิดชื่อชนกันหลายครั้ง
runner.io.dyndep.too_large = ไฟล์ dyndep ที่สร้างขึ้นที่ { $path } มีขนาดเกินขีดจำกัดการตรวจสอบ { $limit } ไบต์

# การวินิจฉัยไฟล์รายการ
manifest.parse = การแจงไฟล์รายการล้มเหลว
manifest.structure_error = โครงสร้างของไฟล์รายการผิดพลาดที่ { $name }: { $details }
manifest.yaml.parse = การแจง YAML ผิดพลาดที่บรรทัด { $line } คอลัมน์ { $column }: { $details }
manifest.yaml.label = YAML ไม่ถูกต้อง
manifest.yaml.hint.tabs = YAML ไม่อนุญาตให้ใช้แท็บ ให้ใช้ช่องว่างในการเยื้อง
manifest.yaml.hint.list_item = รายการย่อยของ YAML ต้องขึ้นต้นด้วย “-” และเยื้องอย่างถูกต้อง
manifest.yaml.hint.expected_colon = ดูเหมือนเป็นรายการของการจับคู่ ขาด “:” หลังคีย์
manifest.yaml.hint.mapping_values = การจับคู่ใน YAML ต้องมีค่าหลัง “:” (หรือบล็อกที่ซ้อนอยู่)
manifest.yaml.hint.invalid_token = โทเคนของ YAML ไม่ถูกต้องหรือไม่คาดคิด
manifest.yaml.hint.escape = โปรดหลีกอักขระแบ็กสแลช หรือลบลำดับหลีกที่ไม่ถูกต้องออก
manifest.env.missing = ยังไม่ได้ตั้งค่าตัวแปรสภาพแวดล้อมที่จำเป็น
manifest.env.invalid_utf8 = ตัวแปรสภาพแวดล้อมมี UTF-8 ที่ไม่ถูกต้อง
manifest.vars.not_object = `vars` ของไฟล์รายการต้องเป็นการจับคู่หรือวัตถุ
manifest.vars.reserved_name = คีย์ `vars` '{ $name }' ของมานิเฟสต์ถูกสงวนไว้สำหรับฟังก์ชันช่วยเทมเพลตในตัว โปรดเปลี่ยนชื่อตัวแปร
manifest.read_failed = อ่านไฟล์รายการที่ { $path } ไม่สำเร็จ
manifest.resolve_workspace_root = ระบุรากของพื้นที่ทำงานไม่สำเร็จ
manifest.workspace_non_utf8 = เส้นทางรากของพื้นที่ทำงาน “{ $path }” ไม่ใช่ UTF-8 ที่ถูกต้อง
manifest.path_non_utf8 = เส้นทางของไฟล์รายการ “{ $manifest }” ไม่ใช่ UTF-8 ที่ถูกต้อง: { $path }
manifest.path_missing_name = เส้นทางไฟล์รายการ “{ $path }” ไม่มีชื่อไฟล์
manifest.open_workspace_failed = เปิดพื้นที่ทำงาน { $workspace } สำหรับไฟล์รายการ { $manifest } ไม่สำเร็จ
manifest.foreach.not_iterable = นิพจน์ `foreach` วนซ้ำไม่ได้
manifest.foreach.serialise_item = ทำให้สมาชิกของ `foreach` เป็นลำดับข้อมูลไม่สำเร็จ
manifest.when.empty = นิพจน์ `when` ต้องไม่ว่างเปล่า
manifest.when.eval_error = ประเมินค่านิพจน์ `when` “{ $expr }” ไม่สำเร็จ
manifest.when.template_error = แสดงแม่แบบ `when` “{ $expr }” ไม่สำเร็จ
manifest.target.vars_not_object = `vars` ของเป้าหมายต้องเป็นวัตถุ แต่ได้ { $value }
manifest.vars.entry_not_object = รายการ `vars` ของไฟล์รายการต้องเป็นวัตถุ
manifest.field_not_string = เขตข้อมูล “{ $field }” ต้องเป็นสายอักขระ
manifest.expression.parse_error = แจงนิพจน์ { $name } ไม่สำเร็จ
manifest.expression.eval_error = ประเมินค่านิพจน์ { $name } ไม่สำเร็จ

# การวินิจฉัยแมโครของไฟล์รายการ
manifest.macro.signature_missing_identifier = ลายเซ็นของแมโครขาดตัวระบุ
manifest.macro.signature_missing_params = ลายเซ็นของแมโครขาดพารามิเตอร์
manifest.macro.compile_failed = คอมไพล์แมโคร { $name } ไม่สำเร็จ
manifest.macro.sequence_invalid = แมโครต้องนิยามเป็นการจับคู่จากชื่อไปยังแม่แบบ
manifest.macro.register_failed = ลงทะเบียนแมโครของไฟล์รายการไม่สำเร็จ
manifest.macro.not_initialised = ยังไม่ได้เตรียมสภาพแวดล้อมของแมโคร
manifest.macro.caller_invalid = ผู้เรียกแมโครต้องเป็นสายอักขระ
manifest.macro.template_load_failed = โหลดแม่แบบของแมโครไม่สำเร็จ
manifest.macro.init_failed = เตรียมสภาพแวดล้อมของแมโครไม่สำเร็จ
manifest.macro.missing = ไม่มีแมโคร { $name }

# ข้อผิดพลาดของรูปแบบ glob ในไฟล์รายการ
manifest.glob.unmatched_brace = รูปแบบ glob ไม่ถูกต้อง “{ $pattern }”: “{ $character }” ที่ตำแหน่ง { $position } ไม่มีคู่
manifest.glob.invalid_pattern = รูปแบบ glob ไม่ถูกต้อง “{ $pattern }”: { $detail }
manifest.glob.unknown_pattern_error = ข้อผิดพลาดของรูปแบบที่ไม่รู้จัก
manifest.glob.io_failed = glob ล้มเหลวสำหรับ “{ $pattern }”: { $detail }
manifest.glob.unknown_io_error = ข้อผิดพลาดรับส่งข้อมูลที่ไม่รู้จัก
manifest.command_list_empty = ฟิลด์ “command” ต้องไม่ว่าง: ระบุสตริงคำสั่งหรือรายการที่ไม่ว่าง

# ข้อผิดพลาดของรูปแทนระดับกลาง
ir.rule_not_found = ไม่พบกฎ “{ $rule }” ที่เป้าหมาย “{ $target }” อ้างถึง
ir.multiple_rules = เป้าหมาย “{ $target }” ต้องอ้างถึงกฎเพียงข้อเดียว แต่ได้ { $rules }
ir.empty_rule = เป้าหมาย “{ $target }” ต้องอ้างถึงกฎหนึ่งข้อ
ir.duplicate_outputs = พบผลลัพธ์ซ้ำกัน: { $outputs }
ir.circular_dependency = พบการพึ่งพาแบบวงกลม: { $cycle }
ir.action_serialisation = ทำให้การกระทำเป็นลำดับข้อมูลไม่สำเร็จ: { $details }
ir.invalid_command = การแทรกค่าในคำสั่งไม่ถูกต้อง: { $snippet }

# ข้อผิดพลาดในการสร้างไฟล์ Ninja
ninja_gen.missing_action = ไม่มีการกระทำ “{ $id }” ที่เส้นเชื่อมของการสร้างอ้างถึง
ninja_gen.format = จัดรูปแบบผลลัพธ์ของไฟล์รายการ Ninja ไม่สำเร็จ
ninja_gen.dyndep_files_required = การดำเนินการนี้ต้องใช้บันเดิล Ninja ที่สร้างขึ้น ให้ใช้ `netsuke build`, `netsuke clean` หรือ `netsuke generate` เพื่อทำให้ไฟล์ dyndep พร้อมใช้งาน
ninja_gen.reserved_output_path = เส้นทาง '{ $path }' สงวนไว้สำหรับสถานะการขึ้นต่อกันแบบลำดับของ Netsuke
ninja_gen.unsupported_path_character = เส้นทาง '{ $path }' มีอักขระเส้นทาง Ninja ที่ไม่รองรับคือ '{ $character }'

# การตรวจสอบรูปแบบโฮสต์
host_pattern.empty = รูปแบบโฮสต์ต้องไม่ว่างเปล่า
host_pattern.contains_scheme = รูปแบบโฮสต์ “{ $pattern }” ต้องไม่มีสกีม URL
host_pattern.contains_slash = รูปแบบโฮสต์ “{ $pattern }” ต้องไม่มี “/”
host_pattern.missing_suffix = รูปแบบโฮสต์ “{ $pattern }” ต้องมีส่วนต่อท้ายหลัง “*.”
host_pattern.empty_label = รูปแบบโฮสต์ “{ $pattern }” มีป้ายกำกับว่างเปล่า
host_pattern.invalid_chars = รูปแบบโฮสต์ “{ $pattern }” มีอักขระที่ไม่ถูกต้อง
host_pattern.invalid_label_edge = ป้ายกำกับของรูปแบบโฮสต์ “{ $pattern }” ต้องไม่ขึ้นต้นหรือลงท้ายด้วย “-”
host_pattern.label_too_long = รูปแบบโฮสต์ “{ $pattern }” มีป้ายกำกับยาวเกิน 63 อักขระ
host_pattern.too_long = รูปแบบโฮสต์ “{ $pattern }” เกินขีดจำกัด 255 อักขระ

# นโยบายเครือข่าย
network_policy.scheme.empty = สกีมต้องไม่ว่างเปล่า
network_policy.scheme.invalid = สกีม “{ $scheme }” มีอักขระที่ไม่ถูกต้อง
network_policy.allowlist.empty = รายชื่อโฮสต์ที่อนุญาตต้องไม่ว่างเปล่า
network_policy.scheme.not_allowed = ไม่อนุญาตให้ใช้สกีม “{ $scheme }”
network_policy.missing_host = URL ไม่มีโฮสต์
network_policy.host.blocked = โฮสต์ “{ $host }” ถูกนโยบายปิดกั้น
network_policy.host.not_allowlisted = โฮสต์ “{ $host }” ไม่อยู่ในรายชื่อที่อนุญาต

# การตั้งค่าไลบรารีมาตรฐาน
stdlib.config.default_fetch_cache_invalid = เส้นทางแคชของ fetch โดยปริยายต้องเป็นเส้นทางสัมพัทธ์
stdlib.config.default_which_cache_invalid = ความจุแคชของ which โดยปริยายต้องเป็นจำนวนบวก
stdlib.config.workspace_root_absolute = เส้นทางรากของพื้นที่ทำงานต้องเป็นเส้นทางสัมบูรณ์
stdlib.config.fetch_response_limit_positive = ขีดจำกัดการตอบสนองของ fetch ต้องเป็นจำนวนบวก
stdlib.config.command_output_limit_positive = ขีดจำกัดการเก็บผลลัพธ์ของคำสั่งต้องเป็นจำนวนบวก
stdlib.config.command_stream_limit_positive = ขีดจำกัดสายข้อมูลของคำสั่งต้องเป็นจำนวนบวก
stdlib.config.which_cache_capacity_positive = ความจุแคชของ which ต้องเป็นจำนวนบวก
stdlib.config.skip_dir_empty = รายการไดเรกทอรีที่ข้ามต้องไม่ว่างเปล่า
stdlib.config.skip_dir_navigation = รายการไดเรกทอรีที่ข้ามต้องไม่มี “..”
stdlib.config.skip_dir_separator = รายการไดเรกทอรีที่ข้ามต้องไม่มีตัวคั่นเส้นทาง
stdlib.config.fetch_cache_empty = เส้นทางแคชของ fetch ต้องไม่ว่างเปล่า
stdlib.config.fetch_cache_not_relative = เส้นทางแคชของ fetch ต้องเป็นเส้นทางสัมพัทธ์ แต่ได้ { $path }
stdlib.config.fetch_cache_escapes = เส้นทางแคชของ fetch ต้องไม่ออกนอกพื้นที่ทำงาน: { $path }
stdlib.config.open_workspace_root = เปิดไดเรกทอรีปัจจุบันเป็นรากของพื้นที่ทำงาน stdlib ไม่สำเร็จ
stdlib.config.resolve_cwd = ระบุไดเรกทอรีปัจจุบันเป็นรากของพื้นที่ทำงาน stdlib ไม่สำเร็จ
stdlib.config.cwd_non_utf8 = ไดเรกทอรีปัจจุบันมีส่วนที่ไม่ใช่ UTF-8: { $path }

# การวินิจฉัยของตัวช่วย fetch
stdlib.fetch.url_invalid = URL ไม่ถูกต้อง “{ $url }”: { $details }
stdlib.fetch.disallowed = ไม่อนุญาตให้ใช้ URL “{ $url }”: { $details }
stdlib.fetch.failed = ดึงข้อมูลจาก “{ $url }” ไม่สำเร็จ: { $details }
stdlib.fetch.cache_read_failed = อ่านรายการแคช “{ $name }” ไม่สำเร็จ: { $details }
stdlib.fetch.cache_open_failed = เปิดรายการแคช “{ $name }” ไม่สำเร็จ: { $details }
stdlib.fetch.response_read_failed = อ่านการตอบสนองจาก “{ $url }” ไม่สำเร็จ: { $details }
stdlib.fetch.response_buffer_overflow = บัฟเฟอร์ล้นขณะอ่าน “{ $url }”
stdlib.fetch.cache_write_failed = เขียนแคชสำหรับ “{ $url }” ไม่สำเร็จ: { $details }
stdlib.fetch.response_limit_exceeded = การตอบสนองจาก “{ $url }” เกินขีดจำกัด { $limit } ไบต์
stdlib.fetch.cache_limit_exceeded = การตอบสนองที่แคชไว้ “{ $name }” เกินขีดจำกัด { $limit } ไบต์
stdlib.fetch.io_failed = การกระทำ “{ $action }” ล้มเหลวสำหรับ { $path }: { $details }
stdlib.fetch.action.sync_cache = ประสานข้อมูลแคชของ fetch
stdlib.fetch.action.create_cache_dir = สร้างไดเรกทอรีแคชของ fetch
stdlib.fetch.action.open_cache_dir = เปิดไดเรกทอรีแคชของ fetch
stdlib.fetch.action.stat_cache = อ่านข้อมูลของรายการแคช fetch
stdlib.fetch.action.open_cache_entry = เปิดรายการแคชของ fetch

# การวินิจฉัยของตัวช่วยด้านคำสั่ง
stdlib.command.location = คำสั่ง “{ $command }” ในแม่แบบ “{ $template }”
stdlib.command.spawn_failed = เริ่ม { $location } ไม่สำเร็จ: { $details }
stdlib.command.io_failed = { $location } ล้มเหลว: { $details }
stdlib.command.closed_input_early = ข้อมูลนำเข้าปิดลงก่อนที่การเขียนไปยังคำสั่งจะเสร็จ
stdlib.command.broken_pipe = ท่อส่งข้อมูลขาดขณะเรียกใช้ { $location }: { $details }
stdlib.command.terminated_by_signal = { $location } ถูกยุติด้วยสัญญาณ
stdlib.command.exited_with_status = { $location } สิ้นสุดด้วยสถานะ { $status }
stdlib.command.output_limit_exceeded = { $location } เกินขีดจำกัด { $mode } ที่ { $limit } ไบต์สำหรับ { $stream }
stdlib.command.timeout = { $location } เกินเวลาที่กำหนด { $seconds } วินาที
stdlib.command.exit_status_suffix = (สถานะการออก { $status })
stdlib.command.signal_suffix = (ถูกยุติด้วยสัญญาณ)
stdlib.command.shell.empty = คำสั่งเชลล์ต้องไม่ว่างเปล่า
stdlib.command.grep.empty_pattern = รูปแบบของ grep ต้องไม่ว่างเปล่า
stdlib.command.grep.flags_not_string = แฟล็กของ grep ต้องเป็นสายอักขระ
stdlib.command.quote.invalid = ใส่เครื่องหมายอัญประกาศให้ { $arg } ไม่สำเร็จ: { $details }
stdlib.command.quote.line_break = อาร์กิวเมนต์ที่มีอักขระขึ้นบรรทัดใหม่หรือปัดแคร่ไม่สามารถใส่เครื่องหมายอัญประกาศได้อย่างปลอดภัย
stdlib.command.input_undefined = ค่าที่นำเข้ายังไม่ได้นิยาม
stdlib.command.tempfile.root_required = การสร้างไฟล์ชั่วคราวของคำสั่งต้องใช้รากของพื้นที่ทำงาน
stdlib.command.tempfile.create_failed = สร้างไฟล์ชั่วคราวของคำสั่งไม่สำเร็จ: { $details }
stdlib.command.options.invalid_utf8 = คีย์ของตัวเลือกคำสั่งต้องเป็น UTF-8 ที่ถูกต้อง
stdlib.command.option.mode_not_string = โหมดการแสดงผลต้องเป็นสายอักขระ
stdlib.command.options.invalid_type = ตัวเลือกของคำสั่งต้องเป็นวัตถุ
stdlib.command.output.mode_unsupported = ไม่รองรับโหมดการแสดงผล “{ $mode }”
stdlib.command.output.mode.capture = การเก็บผลลัพธ์
stdlib.command.output.mode.streaming = การส่งเป็นสายข้อมูล
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# การวินิจฉัยของตัวช่วยด้านเส้นทาง
stdlib.path.io.failed = การกระทำ “{ $action }” ล้มเหลวสำหรับ { $path } ({ $label })
stdlib.path.io.failed_with_detail = การกระทำ “{ $action }” ล้มเหลวสำหรับ { $path }: { $detail }
stdlib.path.io.failed_with_label_and_detail = การกระทำ “{ $action }” ล้มเหลวสำหรับ { $path } ({ $label }): { $detail }
stdlib.path.io.not_found = ไม่พบ
stdlib.path.io.permission_denied = ถูกปฏิเสธสิทธิ์
stdlib.path.io.already_exists = มีอยู่แล้ว
stdlib.path.io.invalid_input = ข้อมูลนำเข้าไม่ถูกต้อง
stdlib.path.io.invalid_data = ข้อมูลไม่ถูกต้อง
stdlib.path.io.timed_out = หมดเวลา
stdlib.path.io.interrupted = ถูกขัดจังหวะ
stdlib.path.io.would_block = จะทำให้เกิดการรอ
stdlib.path.io.write_zero = เขียนได้ศูนย์ไบต์
stdlib.path.io.unexpected_eof = จบไฟล์โดยไม่คาดคิด
stdlib.path.io.broken_pipe = ท่อส่งข้อมูลขาด
stdlib.path.io.connection_refused = การเชื่อมต่อถูกปฏิเสธ
stdlib.path.io.connection_reset = การเชื่อมต่อถูกรีเซ็ต
stdlib.path.io.connection_aborted = การเชื่อมต่อถูกยกเลิก
stdlib.path.io.not_connected = ยังไม่ได้เชื่อมต่อ
stdlib.path.io.addr_in_use = ที่อยู่ถูกใช้งานอยู่
stdlib.path.io.addr_not_available = ที่อยู่ใช้งานไม่ได้
stdlib.path.io.out_of_memory = หน่วยความจำไม่พอ
stdlib.path.io.unsupported = ไม่รองรับ
stdlib.path.io.file_too_large = ไฟล์ใหญ่เกินไป
stdlib.path.io.resource_busy = ทรัพยากรไม่ว่าง
stdlib.path.io.executable_busy = ไฟล์ที่เรียกใช้ได้ไม่ว่าง
stdlib.path.io.deadlock = การติดตายพร้อมกัน
stdlib.path.io.crosses_devices = ข้ามอุปกรณ์
stdlib.path.io.too_many_links = มีลิงก์มากเกินไป
stdlib.path.io.invalid_filename = ชื่อไฟล์ไม่ถูกต้อง
stdlib.path.io.arg_list_too_long = รายการอาร์กิวเมนต์ยาวเกินไป
stdlib.path.io.stale_handle = ตัวชี้ไฟล์เครือข่ายหมดอายุ
stdlib.path.io.storage_full = พื้นที่จัดเก็บเต็ม
stdlib.path.io.not_seekable = เลื่อนตำแหน่งไม่ได้
stdlib.path.io.network_down = เครือข่ายไม่ทำงาน
stdlib.path.io.network_unreachable = เข้าถึงเครือข่ายไม่ได้
stdlib.path.io.host_unreachable = เข้าถึงโฮสต์ไม่ได้
stdlib.path.io.other = ข้อผิดพลาดรับส่งข้อมูล
stdlib.path.action.canonicalize = การทำให้เป็นรูปแบบมาตรฐาน
stdlib.path.action.open_directory = การเปิดไดเรกทอรี
stdlib.path.action.stat = การอ่านข้อมูล
stdlib.path.action.read = การอ่าน
stdlib.path.action.open_file = การเปิดไฟล์
stdlib.path.with_suffix.empty_separator = with_suffix ต้องมีตัวคั่นที่ไม่ว่างเปล่า
stdlib.path.relative_to.mismatch = { $path } ไม่ได้สัมพัทธ์กับ { $root }
stdlib.path.expanduser.unsupported = ไม่รองรับการขยาย ~ สำหรับผู้ใช้รายใดรายหนึ่ง
stdlib.path.expanduser.no_home = ขยาย ~ ไม่ได้: ไม่มีการตั้งค่าตัวแปรสภาพแวดล้อมของไดเรกทอรีบ้าน
stdlib.path.contents.unsupported_encoding = ไม่รองรับการเข้ารหัส “{ $encoding }”
stdlib.path.hash.unsupported_algorithm = ไม่รองรับขั้นตอนวิธีแฮช “{ $algorithm }”
stdlib.path.hash.unsupported_algorithm_legacy = ไม่รองรับขั้นตอนวิธีแฮช “{ $algorithm }” (โปรดเปิดใช้คุณลักษณะ “{ $feature }”)

# การวินิจฉัยของตัวช่วยด้านคอลเลกชัน
stdlib.collections.flatten.expected_sequence = flatten คาดว่าจะพบสมาชิกของลำดับ แต่พบ { $kind }
stdlib.collections.group_by.empty_attribute = group_by ต้องมีแอตทริบิวต์ที่ไม่ว่างเปล่า
stdlib.collections.group_by.unresolved = group_by หา “{ $attr }” ในสมาชิกชนิด { $kind } ไม่พบ

# การวินิจฉัยของตัวช่วยด้านเวลา
stdlib.time.offset.invalid = ค่าเหลื่อมของ now “{ $offset }” ไม่ถูกต้อง: ต้องเป็น “+HH:MM[:SS]” หรือ “Z”
stdlib.time.timedelta.overflow = timedelta ล้นขณะบวก { $component }
stdlib.time.label.weeks = สัปดาห์
stdlib.time.label.days = วัน
stdlib.time.label.hours = ชั่วโมง
stdlib.time.label.minutes = นาที
stdlib.time.label.seconds = วินาที
stdlib.time.label.milliseconds = มิลลิวินาที
stdlib.time.label.microseconds = ไมโครวินาที
stdlib.time.label.nanoseconds = นาโนวินาที

# การวินิจฉัยของตัวช่วย which
stdlib.which.not_found = [netsuke::jinja::which::not_found] ไม่พบคำสั่ง “{ $command }” หลังตรวจรายการใน PATH แล้ว { $count } รายการ ตัวอย่าง: { $preview }
stdlib.which.not_found.hint.cwd_auto = ส่วนที่ว่างเปล่าใน PATH จะถูกละเว้น หากต้องการรวมไดเรกทอรีทำงาน ให้ใช้ cwd_mode="auto"
stdlib.which.not_found.hint.cwd_always = หากต้องการรวมไดเรกทอรีปัจจุบัน ให้ตั้งค่า cwd_mode="always"
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] คำสั่ง “{ $command }” ที่ “{ $path }” ไม่มีอยู่หรือเรียกใช้ไม่ได้
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <ว่างเปล่า>
stdlib.which.path_entry.non_utf8 = รายการที่ { $index } ใน PATH มีอักขระที่ไม่ใช่ UTF-8 Netsuke ต้องใช้เส้นทางแบบ UTF-8
stdlib.which.command.empty = which ต้องใช้สายอักขระที่ไม่ว่างเปล่า
stdlib.which.cwd_mode.invalid = cwd_mode ต้องเป็น “auto” “always” หรือ “never” แต่ได้ “{ $mode }”
stdlib.which.cwd.resolve_failed = ระบุไดเรกทอรีปัจจุบันไม่สำเร็จ: { $details }
stdlib.which.cwd.non_utf8 = ไดเรกทอรีปัจจุบันมีส่วนที่ไม่ใช่ UTF-8
stdlib.which.canonicalize_failed = ทำให้ “{ $path }” เป็นรูปแบบมาตรฐานไม่สำเร็จ: { $details }
stdlib.which.is_executable = ตรวจสอบไม่ได้ว่า “{ $path }” เรียกใช้ได้หรือไม่: { $details }
stdlib.which.canonicalize_non_utf8 = เส้นทางมาตรฐานมีส่วนที่ไม่ใช่ UTF-8
stdlib.which.workspace_non_utf8 = ขณะแก้ปัญหาคำสั่ง “{ $command }” เส้นทางของพื้นที่ทำงานมีส่วนที่ไม่ใช่ UTF-8: { $path }
stdlib.which.walkdir_error = เกิดข้อผิดพลาดขณะท่องพื้นที่ทำงานเพื่อค้นหาคำสั่ง: { $details }

# การลงทะเบียนไลบรารีมาตรฐาน
stdlib.register.open_dir = เปิดไดเรกทอรีปัจจุบันเพื่อลงทะเบียน stdlib ไม่สำเร็จ
stdlib.register.resolve_dir = ระบุไดเรกทอรีปัจจุบันเพื่อลงทะเบียน stdlib ไม่สำเร็จ
stdlib.register.dir_non_utf8 = ไดเรกทอรีปัจจุบันมีส่วนที่ไม่ใช่ UTF-8: { $path }

# การรายงานสถานะสำหรับโหมดการแสดงผลที่เข้าถึงได้
status.state.pending = รอดำเนินการ
status.state.running = กำลังดำเนินการ
status.state.done = เสร็จแล้ว
status.state.failed = ล้มเหลว
status.stage.label = ขั้นที่ { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = งานที่ { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = กำลังอ่านไฟล์รายการ
status.stage.initial_yaml_parsing = กำลังแจงเอกสาร YAML
status.stage.template_expansion = กำลังขยายคำสั่งของแม่แบบ
status.stage.final_rendering = กำลังแปลงกลับและแสดงค่าจากไฟล์รายการ
status.stage.ir_generation_validation = กำลังสร้างและตรวจสอบกราฟการพึ่งพา
status.stage.ninja_synthesis = กำลังสังเคราะห์แผนการสร้างของ Ninja
status.stage.ninja_synthesis_execute = กำลังสังเคราะห์แผนของ Ninja และเรียกใช้ { $tool }
status.stage.graph_rendering = กำลังแสดงผลลัพธ์กราฟ
status.stage.graph_rendering_with_tool = กำลังแสดง { $tool }
status.complete = { $tool } เสร็จสมบูรณ์
status.timing.summary_header = สรุปเวลาตามขั้นตอน:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = เวลารวมของสายงาน: { $duration }
status.tool.build = การสร้าง
status.tool.clean = การล้าง
status.tool.graph = กราฟ
status.tool.graph_html = กราฟ (HTML)
status.tool.generate = การสร้างไฟล์
status.tool.help_targets = ความช่วยเหลือเป้าหมาย

# ข้อความของตัวแสดงกราฟเป็น HTML
graph.html.title = กราฟการสร้างของ Netsuke
graph.html.heading = กราฟการสร้างของ Netsuke
graph.html.description = กราฟการสร้างที่แสดงโดย Netsuke
graph.html.outline.summary = เป้าหมายและการพึ่งพา (โครงร่างข้อความ)
graph.html.outline.no_inputs = ไม่มีข้อมูลนำเข้า
graph.html.noscript.notice = JavaScript ถูกปิดอยู่ โครงร่างข้อความด้านบนคือกราฟทั้งหมด ถัดไปเป็นซอร์ส DOT

# คำนำหน้าเชิงความหมายสำหรับการแสดงผลที่เข้าถึงได้
semantic.prefix.error = ข้อผิดพลาด:
semantic.prefix.warning = คำเตือน:
semantic.prefix.success = สำเร็จ:
semantic.prefix.info = ข้อมูล:
semantic.prefix.timing = เวลา:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# ตัวอย่างรูปพหูพจน์สำหรับผู้แปล
# ภาษาไทยไม่มีการผันรูปพหูพจน์ CLDR จึงมีหมวดเดียวคือ `other`
example.files_processed = { $count ->
   *[other] ประมวลผลแล้ว { $count } ไฟล์
}

example.errors_found = { $count ->
    [0] ไม่พบข้อผิดพลาด
   *[other] พบข้อผิดพลาด { $count } รายการ
}
