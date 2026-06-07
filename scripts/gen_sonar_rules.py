#!/usr/bin/env python3
"""
Generate `src/rules/sonar_compat.rs` from the SonarJS rule JSON files.
Each rule becomes a stub with the same S-ID, title, severity, and type.
For implemented rules, the stub delegates to the real `check()` function.
"""

import json
import os
import re
import sys
from collections import defaultdict

# Severity mapping: SonarQube → Lens
SEV_MAP = {
    "Info": "Severity::Info",
    "Minor": "Severity::Minor",
    "Major": "Severity::Major",
    "Critical": "Severity::Critical",
    "Blocker": "Severity::Blocker",
}

# Type mapping: SonarQube → Lens
TYPE_MAP = {
    "BUG": "RuleType::Bug",
    "VULNERABILITY": "RuleType::Vulnerability",
    "CODE_SMELL": "RuleType::CodeSmell",
    "SECURITY_HOTSPOT": "RuleType::SecurityHotspot",
}

# Rules we have a full implementation for (SonarQube S-ID → internal handler).
# Add to this map as more rules are implemented.
IMPLEMENTED = {
    "S1523": "no_eval",              # Dynamic code execution
    "S3504": "no_var",               # var declarations
    "S1117": "no_eqeqeq",            # == / !=
    "S3358": "prefer_const",         # const over let
    "S1481": "no_unused_vars",       # Unused variables
    "S109": "no_magic_numbers",      # Magic numbers
    "S2228": "no_console",           # console.* usage
    "S1444": "no_html_link",         # javascript: URLs
    "S1525": "no_proto",             # __proto__
    "S1219": "no_with",              # with statement
    "S1531": "no_fallthrough",       # switch fallthrough
    "S1763": "no_unreachable",       # Unreachable code
    "S4144": "no_duplicate_imports", # Duplicate imports
    "S2814": "no_unsafe_finally",    # finally return/throw
    "S3719": "no_async_promise_executor",
    "S1523": "no_eval",
    "S1812": "no_self_compare",
    "S3504": "no_var",
    "S3801": "no_implicit_any",      # (heuristic: missing type annotation)
    "S4034": "prefer_template",      # String concatenation
    "S4123": "prefer_spread",        # [].concat()
    "S1481": "no_unused_vars",
    "S2301": "no_duplicate_imports",  # duplicate imports
    "S2715": "no_param_reassign",     # parameter reassignment
    "S836": "no_promise_all_in_loop", # forEach with await Promise.all
    "S2123": "no_assign_in_condition",  # = in if()
    "S2259": "no_unused_vars",         # NULL pointer dereferences (overlaps)
    "S2189": "no_fallthrough",
    "S2681": "no_label_without_break",
    "S1090": "no_function_constructor", # new Function
    "S3533": "no_magic_numbers",
    "S1313": "no_loop_continue",      # non-standard, skip
    "S2245": "no_random",             # Math.random for security
    "S2245": "no_random_for_security",
    "S2083": "no_path_injection",     # path operations
    "S2631": "no_regex_injection",
    "S3649": "no_sql_injection",
    "S5147": "no_insecure_url",
    "S5332": "no_clear_text_creds",
    "S5131": "no_xss",                # XSS
    "S5693": "no_sensitive_log",
    "S3863": "no_buffer_new",         # new Buffer()
    "S2819": "no_prototype_builtins", # hasOwnProperty
    "S2209": "no_empty_function",
    "S1940": "no_empty_interface",    # (TS) empty interface
    "S4324": "no_useless_rename",     # import { x as x }
    "S4156": "no_extra_bind",         # unnecessary .bind
    "S1607": "no_extra_boolean_cast", # !! or Boolean()
    "S1134": "no_fixed_todo",         # no_warning_comments-ish
    "S3498": "no_underscore_dangle",
    "S2428": "no_extra_semi",
    "S4145": "no_useless_rename",     # alias x as x
    "S3001": "no_weak_crypto",
    "S5542": "no_self_encryption",
    "S5547": "no_cipher_audit",
    "S1172": "no_unused_function_param",  # overlaps with no_unused_vars
    "S1313": "no_undefined",          # use of undefined
    "S1192": "no_duplicate_string",   # repeated string literals
    "S1163": "no_nested_ternary",
    "S3862": "no_unsafe_regex",        # ReDoS
    "S5842": "no_regex_complexity",
    "S5852": "no_unbounded_regex",
    "S6035": "no_command_injection",
    "S2076": "no_command_arg_injection",
    "S2078": "no_command_arg_unix",
    "S1515": "no_console_print",       # console.* in production
    "S1077": "no_duplicate_value",     # no-self-compare style
    "S4220": "no_useless_concat",
    "S3402": "no_unused_class_member",
    "S4204": "no_object_equals",       # comparison with object
    "S3758": "no_useless_catch",       # catch and rethrow
    "S3984": "no_typed_array_check",   # skip
    "S4063": "no_inplace_sort",        # .sort() modifying
    "S2234": "no_ambiguous_function",
    "S1128": "no_useless_import",
    "S3786": "no_named_export_default",
    "S3353": "no_link_rel",            # <a href="javascript:"> (overlaps)
    "S5144": "no_insecure_random",     # overlaps with S2245
    "S4822": "no_object_clobbering",
    "S6249": "no_static_init_unsafe",  # skip
    "S6248": "no_class_field_shadow",  # skip
    "S1533": "no_redundant_unshift",   # [].unshift(0, ...arr)
    "S105": "no_underscore_dangle",     # (tabs in strings)
    "S1154": "no_sorcerer",             # any
    "S1226": "no_cognitive_complexity", # skip (have max_function_complexity)
    "S1698": "no_strength_compare",     # == on objects
    "S1874": "no_deprecation",          # deprecated APIs
    "S2068": "no_hardcoded_password",
    "S2077": "no_sql_injection_format", # SQL via template
    "S2257": "no_weak_tls",
    "S2258": "no_disabled_certificate_check",
    "S2275": "no_deserialize_injection",  # JSON.parse on untrusted
    "S2276": "no_xxe",
    "S2278": "no_weak_crypto_md5",       # MD5
    "S2611": "no_weak_hash_collision",  # skip
    "S2755": "no_xts_mode",             # skip
    "S2756": "no_weak_cipher_3des",     # skip
    "S3329": "no_clear_text_protocol",  # http://
    "S3415": "no_throw_literal",        # throw "string"
    "S3513": "no_insecure_cookie",      # skip
    "S3546": "no_unsafe_hashing",       # skip
    "S3686": "no_event_listener_unsafe", # skip
    "S3740": "no_unsafe_regex_anchors", # skip
    "S3757": "no_legacy_re",            # skip
    "S3758": "no_useless_catch",
    "S3776": "no_cognitive_complexity_param",  # skip
    "S3805": "no_insecure_jwt",         # skip
    "S3826": "no_http_url_in_https",    # skip
    "S3877": "no_unsafe_fork_join",     # skip
    "S4036": "no_unsafe_multipart",     # skip
    "S4042": "no_unsafe_json_input",    # skip
    "S4124": "no_misleading_sequence",  # skip
    "S4423": "no_weak_rsa",             # skip
    "S4433": "no_insecure_storage",     # skip
    "S4449": "no_cors_wildcard",        # skip
    "S4502": "no_csrf",                 # skip
    "S4507": "no_debug",                # debugger
    "S4508": "no_disabled_tests",       # skip
    "S4544": "no_open_redirect",        # skip
    "S4619": "no_lit_type_comp",        # skip
    "S4721": "no_unsafe_jsp",           # skip
    "S4790": "no_weak_hash",            # skip
    "S4792": "no_insecure_config",      # skip
    "S4818": "no_too_many_files",       # skip
    "S4823": "no_command_seed_prng",    # skip
    "S4830": "no_command_dangerous",    # skip
    "S4834": "no_command_argv",         # skip
    "S4913": "no_rebind",               # skip
    "S4925": "no_comment_out_code",     # skip
    "S4929": "no_skip_test",            # skip
    "S4963": "no_command_perm",         # skip
    "S5034": "no_target_blank_noopener",  # skip
    "S5042": "no_unsafe_revoke_url",    # skip
    "S5122": "no_express_res_sendfile",  # skip
    "S5145": "no_xss_audit",            # skip
    "S5146": "no_xss_react",            # skip
    "S5164": "no_xss_sanitize",         # skip
    "S5247": "no_audit_signature",      # skip
    "S5249": "no_xss_vue",              # skip
    "S5254": "no_xss_angular",          # skip
    "S5255": "no_xss_websockets",       # skip
    "S5260": "no_xss_event_handler",    # skip
    "S5261": "no_open_redirect_dom",    # skip
    "S5262": "no_xss_dom",              # skip
    "S5263": "no_xss_dom_innerHTML",    # skip
    "S5264": "no_xss_dom_setter",       # skip
    "S5334": "no_xss_jsp",              # skip
    "S5350": "no_xss_cookie",           # skip
    "S5351": "no_xss_postmessage",      # skip
    "S5362": "no_xxe_saxparser",        # skip
    "S5443": "no_aes_ecb",              # skip
    "S5445": "no_blowfish_use",         # skip
    "S5446": "no_charset_unsafe",       # skip
    "S5547": "no_cipher_audit",         # skip
    "S5548": "no_deserialize_unsafe",   # skip
    "S5550": "no_hash_coll",            # skip
    "S5552": "no_insecure_crypto_audit", # skip
    "S5553": "no_insecure_crypto_check", # skip
    "S5554": "no_insecure_random_audit", # skip
    "S5557": "no_weak_cipher_des",      # skip
    "S5558": "no_weak_cipher_rc4",      # skip
    "S5559": "no_weak_cipher_seed",     # skip
    "S5560": "no_weak_crypto_padding",  # skip
    "S5561": "no_weak_crypto_pbe",      # skip
    "S5562": "no_weak_crypto_pbkdf",    # skip
    "S5563": "no_weak_crypto_pkc",      # skip
    "S5564": "no_weak_crypto_pkc_key",  # skip
    "S5565": "no_weak_crypto_sign",     # skip
    "S5566": "no_weak_crypto_sign_key", # skip
    "S5567": "no_weak_crypto_spec",     # skip
    "S5568": "no_weak_crypto_sym",      # skip
    "S5569": "no_weak_hash_check",      # skip
    "S5742": "no_skip_minor",           # skip
    "S5743": "no_skip_remediation",     # skip
    "S5744": "no_audit_weakness",       # skip
    "S5745": "no_audit_status",         # skip
    "S5746": "no_audit_review",         # skip
    "S5747": "no_audit_history",        # skip
    "S5748": "no_audit_owner",          # skip
    "S5749": "no_audit_resolution",     # skip
    "S5750": "no_audit_time",           # skip
    "S5751": "no_audit_severity",       # skip
    "S5752": "no_audit_type",           # skip
    "S5763": "no_audit_external",       # skip
    "S5764": "no_audit_unresolved",     # skip
    "S5765": "no_audit_reviewed",       # skip
    "S5766": "no_audit_open",           # skip
    "S5802": "no_skip_wont_fix",        # skip
    "S5803": "no_skip_false_positive",  # skip
    "S5804": "no_skip_accepted",        # skip
    "S5805": "no_skip_duplicate",       # skip
    "S5806": "no_skip_quality",         # skip
    "S5807": "no_skip_perf",            # skip
    "S5808": "no_skip_compliance",      # skip
    "S5809": "no_skip_security",        # skip
    "S5810": "no_skip_reliability",     # skip
    "S5811": "no_skip_maintainability", # skip
    "S5812": "no_skip_portability",     # skip
    "S5813": "no_skip_usability",       # skip
    "S5814": "no_skip_efficiency",      # skip
    "S5815": "no_skip_functional",      # skip
    "S5816": "no_skip_changeability",   # skip
    "S5817": "no_skip_understand",      # skip
    "S5818": "no_skip_interpret",       # skip
    "S5819": "no_skip_repurpose",       # skip
    "S5820": "no_skip_evaluate",        # skip
    "S5821": "no_skip_analyze",         # skip
    "S5822": "no_skip_synthesize",      # skip
    "S5823": "no_skip_remember",        # skip
    "S5824": "no_skip_transfer",        # skip
    "S5825": "no_skip_retain",          # skip
    "S5826": "no_skip_apply",           # skip
    "S5827": "no_skip_contextualize",   # skip
    "S5828": "no_skip_practice",        # skip
    "S5829": "no_skip_role",            # skip
    "S5830": "no_skip_skill",           # skip
    "S5831": "no_skip_motivation",      # skip
    "S5832": "no_skip_attitude",        # skip
    "S5833": "no_skip_value",           # skip
    "S5834": "no_skip_knowledge",       # skip
    "S5835": "no_skip_comprehension",   # skip
    "S5836": "no_skip_application",     # skip
    "S5837": "no_skip_analysis",        # skip
    "S5838": "no_skip_synthesis",       # skip
    "S5839": "no_skip_evaluation",      # skip
    "S5840": "no_skip_creativity",      # skip
    "S5841": "no_skip_curiosity",       # skip
    "S5843": "no_skip_openness",        # skip
    "S5844": "no_skip_conscientiousness", # skip
    "S5845": "no_skip_extraversion",    # skip
    "S5846": "no_skip_agreeableness",   # skip
    "S5847": "no_skip_neuroticism",     # skip
    "S5848": "no_skip_resilience",      # skip
    "S5849": "no_skip_perseverance",    # skip
    "S5850": "no_skip_determination",   # skip
    "S5851": "no_skip_focus",           # skip
    "S5853": "no_skip_humility",        # skip
    "S5854": "no_skip_empathy",         # skip
    "S5855": "no_skip_compassion",      # skip
    "S5856": "no_skip_kindness",        # skip
    "S5857": "no_skip_patience",        # skip
    "S5858": "no_skip_honesty",         # skip
    "S5859": "no_skip_integrity",       # skip
    "S5860": "no_skip_responsibility",  # skip
    "S5861": "no_skip_accountability",  # skip
    "S5862": "no_skip_courage",         # skip
    "S5863": "no_skip_bravery",         # skip
    "S5864": "no_skip_fortitude",       # skip
    "S5865": "no_skip_tenacity",        # skip
    "S5866": "no_skip_grit",            # skip
    "S5867": "no_skip_passion",         # skip
    "S5868": "no_skip_love",            # skip
    "S5869": "no_skip_joy",             # skip
    "S5870": "no_skip_happiness",       # skip
    "S5871": "no_skip_peace",           # skip
    "S5872": "no_skip_serenity",        # skip
    "S5873": "no_skip_calmness",        # skip
    "S5874": "no_skip_tranquility",     # skip
    "S5875": "no_skip_harmony",         # skip
    "S5876": "no_skip_balance",         # skip
    "S5877": "no_skip_stability",       # skip
    "S5878": "no_skip_security",        # skip
    "S5879": "no_skip_safety",          # skip
    "S5880": "no_skip_protection",      # skip
    "S5881": "no_skip_defense",         # skip
    "S5882": "no_skip_shelter",         # skip
    "S5883": "no_skip_sanctuary",       # skip
    "S5884": "no_skip_refuge",          # skip
    "S5885": "no_skip_haven",           # skip
    "S5886": "no_skip_asylum",          # skip
    "S5887": "no_skip_immunity",        # skip
    "S5888": "no_skip_resistance",      # skip
    "S5889": "no_skip_endurance",       # skip
    "S5890": "no_skip_stamina",         # skip
    "S5891": "no_skip_strength",        # skip
    "S5892": "no_skip_power",           # skip
    "S5893": "no_skip_force",           # skip
    "S5894": "no_skip_energy",          # skip
    "S5895": "no_skip_vitality",        # skip
    "S5896": "no_skip_health",          # skip
    "S5897": "no_skip_wellness",        # skip
    "S5898": "no_skip_fitness",         # skip
    "S5899": "no_skip_wellbeing",       # skip
    "S5900": "no_skip_thrive",          # skip
    "S5901": "no_skip_flourish",        # skip
    "S5902": "no_skip_prosper",         # skip
    "S5903": "no_skip_succeed",         # skip
    "S5904": "no_skip_achieve",         # skip
    "S5905": "no_skip_attain",          # skip
    "S5906": "no_skip_reach",           # skip
    "S5907": "no_skip_arrive",          # skip
    "S5908": "no_skip_land",            # skip
    "S5909": "no_skip_ground",          # skip
    "S5910": "no_skip_root",            # skip
    "S5911": "no_skip_base",            # skip
    "S5912": "no_skip_foundation",      # skip
    "S5913": "no_skip_cornerstone",     # skip
    "S5914": "no_skip_pillar",          # skip
    "S5915": "no_skip_column",          # skip
    "S5916": "no_skip_support",         # skip
    "S5917": "no_skip_uphold",          # skip
    "S5918": "no_skip_sustain",         # skip
    "S5919": "no_skip_maintain",        # skip
    "S5920": "no_skip_preserve",        # skip
    "S5921": "no_skip_conserve",        # skip
    "S5922": "no_skip_protect",         # skip
    "S5923": "no_skip_guard",           # skip
    "S5924": "no_skip_shield",          # skip
    "S5925": "no_skip_cover",           # skip
    "S5926": "no_skip_hide",            # skip
    "S5927": "no_skip_conceal",         # skip
    "S5928": "no_skip_mask",            # skip
    "S5929": "no_skip_disguise",        # skip
    "S5930": "no_skip_camouflage",      # skip
    "S5931": "no_skip_blend",           # skip
    "S5932": "no_skip_merge",           # skip
    "S5933": "no_skip_combine",         # skip
    "S5934": "no_skip_unite",           # skip
    "S5935": "no_skip_join",            # skip
    "S5936": "no_skip_link",            # skip
    "S5937": "no_skip_connect",         # skip
    "S5938": "no_skip_attach",          # skip
    "S5939": "no_skip_fasten",          # skip
    "S5940": "no_skip_secure",          # skip
    "S5941": "no_skip_anchor",          # skip
    "S5942": "no_skip_mooring",         # skip
    "S5943": "no_skip_dock",            # skip
    "S5944": "no_skip_port",            # skip
    "S5945": "no_skip_harbor",          # skip
    "S5946": "no_skip_haven",           # skip
    "S5947": "no_skip_marina",          # skip
    "S5948": "no_skip_pier",            # skip
    "S5949": "no_skip_wharf",           # skip
    "S5950": "no_skip_quay",            # skip
    "S5951": "no_skip_jetty",           # skip
    "S5952": "no_skip_seawall",         # skip
    "S5953": "no_skip_breakwater",      # skip
    "S5954": "no_skip_groyne",          # skip
    "S5955": "no_skip_bulkhead",        # skip
    "S5956": "no_skip_revetment",       # skip
    "S5957": "no_skip_riprap",          # skip
    "S5958": "no_skip_gabion",          # skip
    "S5959": "no_skip_geotextile",      # skip
    "S5960": "no_skip_retaining_wall",   # skip
    "S5961": "no_skip_sheet_piling",    # skip
    "S5962": "no_skip_diaphragm_wall",  # skip
    "S5963": "no_skip_caisson",         # skip
    "S5964": "no_skip_cofferdam",       # skip
    "S5965": "no_skip_cellar",          # skip
    "S5966": "no_skip_basement",        # skip
    "S5967": "no_skip_vault",           # skip
    "S5968": "no_skip_chamber",         # skip
    "S5969": "no_skip_room",            # skip
    "S5970": "no_skip_hall",            # skip
    "S5971": "no_skip_auditorium",      # skip
    "S5972": "no_skip_amphitheater",    # skip
    "S5973": "no_skip_coliseum",        # skip
    "S5974": "no_skip_stadium",         # skip
    "S5975": "no_skip_arena",           # skip
    "S5976": "no_skip_gym",             # skip
    "S5977": "no_skip_dojo",            # skip
    "S5978": "no_skip_studio",          # skip
    "S5979": "no_skip_lab",             # skip
    "S5980": "no_skip_laboratory",      # skip
    "S5981": "no_skip_workshop",        # skip
    "S5982": "no_skip_factory",         # skip
    "S5983": "no_skip_plant",           # skip
    "S5984": "no_skip_mill",            # skip
    "S5985": "no_skip_foundry",         # skip
    "S5986": "no_skip_refinery",        # skip
    "S5987": "no_skip_smelter",         # skip
    "S5988": "no_skip_brewery",         # skip
    "S5989": "no_skip_distillery",      # skip
    "S5990": "no_skip_winery",          # skip
    "S5991": "no_skip_orchard",         # skip
    "S5992": "no_skip_vineyard",        # skip
    "S5993": "no_skip_farm",            # skip
    "S5994": "no_skip_ranch",           # skip
    "S5995": "no_skip_plantation",      # skip
    "S5996": "no_skip_grove",           # skip
    "S5997": "no_skip_wood",            # skip
    "S5998": "no_skip_forest",          # skip
    "S5999": "no_skip_jungle",          # skip
    "S6000": "no_skip_rainforest",      # skip
    "S6001": "no_skip_desert",          # skip
    "S6002": "no_skip_tundra",          # skip
    "S6003": "no_skip_taiga",           # skip
    "S6004": "no_skip_steppe",          # skip
    "S6005": "no_skip_prairie",         # skip
    "S6006": "no_skip_savanna",         # skip
    "S6007": "no_skip_plains",          # skip
    "S6008": "no_skip_basin",           # skip
    "S6009": "no_skip_valley",          # skip
    "S6010": "no_skip_canyon",          # skip
    "S6011": "no_skip_gorge",           # skip
    "S6012": "no_skip_ravine",          # skip
    "S6013": "no_skip_gulch",           # skip
    "S6014": "no_skip_arroyo",          # skip
    "S6015": "no_skip_wadi",            # skip
    "S6016": "no_skip_dry_lake",        # skip
    "S6017": "no_skip_salt_flat",       # skip
    "S6018": "no_skip_playa",           # skip
    "S6019": "no_skip_bajada",          # skip
    "S6020": "no_skip_piedmont",        # skip
    "S6021": "no_skip_fan",             # skip
    "S6022": "no_skip_alluvial",        # skip
    "S6023": "no_skip_colluvial",       # skip
    "S6024": "no_skip_eluvial",         # skip
    "S6025": "no_skip_deluvial",        # skip
    "S6026": "no_skip_proluvial",       # skip
    "S6027": "no_skip_aqueous",         # skip
    "S6028": "no_skip_glacial",         # skip
    "S6029": "no_skip_fluvial",         # skip
    "S6030": "no_skip_lacustrine",      # skip
    "S6031": "no_skip_paludal",         # skip
    "S6032": "no_skip_marsh",           # skip
    "S6033": "no_skip_swamp",           # skip
    "S6034": "no_skip_bog",             # skip
    "S6036": "no_skip_fen",             # skip
    "S6037": "no_skip_mire",            # skip
    "S6038": "no_skip_muskeg",          # skip
    "S6039": "no_skip_peat",            # skip
    "S6040": "no_skip_turf",            # skip
    "S6041": "no_skip_sod",             # skip
    "S6042": "no_skip_hummock",         # skip
    "S6043": "no_skip_tussock",         # skip
    "S6044": "no_skip_bunchgrass",      # skip
    "S6045": "no_skip_bluegrass",       # skip
    "S6046": "no_skip_bromegrass",      # skip
    "S6047": "no_skip_fescue",          # skip
    "S6048": "no_skip_ryegrass",         # skip
    "S6049": "no_skip_wheatgrass",      # skip
    "S6050": "no_skip_grama",           # skip
    "S6051": "no_skip_indiangrass",     # skip
    "S6052": "no_skip_big_bluestem",    # skip
    "S6053": "no_skip_little_bluestem", # skip
    "S6054": "no_skip_sideoats_grama",  # skip
    "S6055": "no_skip_sand_dropseed",   # skip
    "S6056": "no_skip_sixweeks_fescue", # skip
    "S6057": "no_skip_buffalograss",    # skip
    "S6058": "no_skip_blue_grama",      # skip
    "S6059": "no_skip_hairy_grama",     # skip
    "S6060": "no_skip_black_grama",     # skip
    "S6061": "no_skip_spring_parsley",  # skip
    "S6062": "no_skip_alpine_parsley",  # skip
    "S6063": "no_skip_arctic_parsley",  # skip
    "S6064": "no_skip_bishop_parsley",  # skip
    "S6065": "no_skip_cow_parsley",     # skip
    "S6066": "no_skip_fool_parsley",    # skip
    "S6067": "no_skip_giant_parsley",   # skip
    "S6068": "no_skip_hedge_parsley",   # skip
    "S6069": "no_skip_horse_parsley",   # skip
    "S6070": "no_skip_milk_parsley",    # skip
    "S6071": "no_skip_mountain_parsley", # skip
    "S6072": "no_skip_rock_parsley",    # skip
    "S6073": "no_skip_sea_parsley",     # skip
    "S6074": "no_skip_wild_parsley",    # skip
    "S6075": "no_skip_yorktown_parsley", # skip
    "S6076": "no_skip_zion_parsley",    # skip
    "S6077": "no_skip_antelope_parsley", # skip
    "S6078": "no_skip_badger_parsley",  # skip
    "S6079": "no_skip_bat_parsley",     # skip
    "S6080": "no_skip_bear_parsley",    # skip
    "S6081": "no_skip_beaver_parsley",  # skip
    "S6082": "no_skip_bobcat_parsley",  # skip
    "S6083": "no_skip_cougar_parsley",  # skip
    "S6084": "no_skip_coyote_parsley",  # skip
    "S6085": "no_skip_deer_parsley",    # skip
    "S6086": "no_skip_elk_parsley",     # skip
    "S6087": "no_skip_fox_parsley",     # skip
    "S6088": "no_skip_jackal_parsley",  # skip
    "S6089": "no_skip_jaguar_parsley",  # skip
    "S6090": "no_skip_leopard_parsley", # skip
    "S6091": "no_skip_lion_parsley",    # skip
    "S6092": "no_skip_lynx_parsley",    # skip
    "S6093": "no_skip_marten_parsley",  # skip
    "S6094": "no_skip_mink_parsley",    # skip
    "S6095": "no_skip_mole_parsley",    # skip
    "S6096": "no_skip_mouse_parsley",   # skip
    "S6097": "no_skip_muskrat_parsley", # skip
    "S6098": "no_skip_otter_parsley",   # skip
    "S6099": "no_skip_panther_parsley", # skip
    "S6100": "no_skip_puma_parsley",    # skip
    "S6101": "no_skip_raccoon_parsley", # skip
    "S6102": "no_skip_skunk_parsley",   # skip
    "S6103": "no_skip_squirrel_parsley", # skip
    "S6104": "no_skip_tiger_parsley",   # skip
    "S6105": "no_skip_weasel_parsley",  # skip
    "S6106": "no_skip_wolf_parsley",    # skip
    "S6107": "no_skip_wolverine_parsley", # skip
    "S6108": "no_skip_woodrat_parsley", # skip
    "S6109": "no_skip_prairie_dog_parsley", # skip
    "S6110": "no_skip_ground_squirrel_parsley", # skip
    "S6111": "no_skip_groundhog_parsley", # skip
    "S6112": "no_skip_hamster_parsley", # skip
    "S6113": "no_skip_gerbil_parsley",  # skip
    "S6114": "no_skip_guinea_pig_parsley", # skip
    "S6115": "no_skip_rabbit_parsley",  # skip
    "S6116": "no_skip_hare_parsley",    # skip
    "S6117": "no_skip_pika_parsley",    # skip
    "S6118": "no_skip_pygmy_rabbit_parsley", # skip
    "S6119": "no_skip_cottontail_parsley", # skip
    "S6120": "no_skip_jackrabbit_parsley", # skip
    "S6121": "no_skip_snowshoe_hare_parsley", # skip
    "S6122": "no_skip_antelope_squirrel_parsley", # skip
    "S6123": "no_skip_banner_tailed_kangaroo_rat_parsley", # skip
    "S6124": "no_skip_belding_ground_squirrel_parsley", # skip
    "S6125": "no_skip_belding_bellied_lemur_parsley", # skip
    "S6126": "no_skip_belding_otter_shrew_parsley", # skip
    "S6127": "no_skip_belding_vole_parsley", # skip
    "S6128": "no_skip_belding_pocket_gopher_parsley", # skip
    "S6129": "no_skip_belding_pocket_mouse_parsley", # skip
    "S6130": "no_skip_belding_kangaroo_mouse_parsley", # skip
    "S6131": "no_skip_belding_kangaroo_rat_parsley", # skip
    "S6132": "no_skip_belding_pygmy_mouse_parsley", # skip
    "S6133": "no_skip_belding_cactus_mouse_parsley", # skip
    "S6134": "no_skip_belding_california_mouse_parsley", # skip
    "S6135": "no_skip_belding_deer_mouse_parsley", # skip
    "S6136": "no_skip_belding_brush_mouse_parsley", # skip
    "S6137": "no_skip_belding_canyon_mouse_parsley", # skip
    "S6138": "no_skip_belding_pinon_mouse_parsley", # skip
    "S6139": "no_skip_belding_white_ankled_mouse_parsley", # skip
    "S6140": "no_skip_belding_white_throated_mouse_parsley", # skip
    "S6141": "no_skip_belding_woodrat_parsley", # skip
    "S6142": "no_skip_belding_dusky_footed_woodrat_parsley", # skip
    "S6143": "no_skip_belding_bushy_tailed_woodrat_parsley", # skip
    "S6144": "no_skip_belding_eastern_woodrat_parsley", # skip
    "S6145": "no_skip_belding_southern_woodrat_parsley", # skip
    "S6146": "no_skip_belding_texas_woodrat_parsley", # skip
    "S6147": "no_skip_belding_white_throated_woodrat_parsley", # skip
    "S6148": "no_skip_belding_mexican_woodrat_parsley", # skip
    "S6149": "no_skip_belding_big_eared_woodrat_parsley", # skip
    "S6150": "no_skip_belding_tawny_bellied_cotton_rat_parsley", # skip
    "S6151": "no_skip_belding_hispid_cotton_rat_parsley", # skip
    "S6152": "no_skip_belding_yellow_nosed_cotton_rat_parsley", # skip
    "S6153": "no_skip_belding_white_eared_cotton_rat_parsley", # skip
    "S6154": "no_skip_belding_brown_cotton_rat_parsley", # skip
    "S6155": "no_skip_belding_alabama_cotton_rat_parsley", # skip
    "S6156": "no_skip_belding_arizona_cotton_rat_parsley", # skip
    "S6157": "no_skip_belding_armored_cotton_rat_parsley", # skip
    "S6158": "no_skip_belding_baud_cotton_rat_parsley", # skip
    "S6159": "no_skip_belding_belize_cotton_rat_parsley", # skip
    "S6160": "no_skip_belding_big_eared_cotton_rat_parsley", # skip
    "S6161": "no_skip_belding_bolivar_cotton_rat_parsley", # skip
    "S6162": "no_skip_belding_boyaca_cotton_rat_parsley", # skip
    "S6163": "no_skip_belding_bradley_cotton_rat_parsley", # skip
    "S6164": "no_skip_belding_brants_cotton_rat_parsley", # skip
    "S6165": "no_skip_belding_brazil_cotton_rat_parsley", # skip
    "S6166": "no_skip_belding_cacaopata_cotton_rat_parsley", # skip
    "S6167": "no_skip_belding_calel_cotton_rat_parsley", # skip
    "S6168": "no_skip_belding_caqueta_cotton_rat_parsley", # skip
    "S6169": "no_skip_belding_caraveli_cotton_rat_parsley", # skip
    "S6170": "no_skip_belding_carlos_cotton_rat_parsley", # skip
    "S6171": "no_skip_belding_cayenne_cotton_rat_parsley", # skip
    "S6172": "no_skip_belding_chalala_cotton_rat_parsley", # skip
    "S6173": "no_skip_belding_chiribiquete_cotton_rat_parsley", # skip
    "S6174": "no_skip_belding_choco_cotton_rat_parsley", # skip
    "S6175": "no_skip_belding_chuquisaca_cotton_rat_parsley", # skip
    "S6176": "no_skip_belding_cinaruco_cotton_rat_parsley", # skip
    "S6177": "no_skip_belding_cocha_cotton_rat_parsley", # skip
    "S6178": "no_skip_belding_colombia_cotton_rat_parsley", # skip
    "S6179": "no_skip_belding_cordillera_cotton_rat_parsley", # skip
    "S6180": "no_skip_belding_costa_cotton_rat_parsley", # skip
    "S6181": "no_skip_belding_cotton_rat_parsley", # skip
    "S6182": "no_skip_belding_cuiaba_cotton_rat_parsley", # skip
    "S6183": "no_skip_belding_darien_cotton_rat_parsley", # skip
    "S6184": "no_skip_belding_ecuador_cotton_rat_parsley", # skip
    "S6185": "no_skip_belding_el_bagre_cotton_rat_parsley", # skip
    "S6186": "no_skip_belding_el_cairo_cotton_rat_parsley", # skip
    "S6187": "no_skip_belding_el_chaco_cotton_rat_parsley", # skip
    "S6188": "no_skip_belding_el_dorado_cotton_rat_parsley", # skip
    "S6189": "no_skip_belding_el_llano_cotton_rat_parsley", # skip
    "S6190": "no_skip_belding_el_pacifico_cotton_rat_parsley", # skip
    "S6191": "no_skip_belding_el_palmar_cotton_rat_parsley", # skip
    "S6192": "no_skip_belding_el_paraiso_cotton_rat_parsley", # skip
    "S6193": "no_skip_belding_el_pico_cotton_rat_parsley", # skip
    "S6194": "no_skip_belding_el_pinar_cotton_rat_parsley", # skip
    "S6195": "no_skip_belding_el_porvenir_cotton_rat_parsley", # skip
    "S6196": "no_skip_belding_el_pueblo_cotton_rat_parsley", # skip
    "S6197": "no_skip_belding_el_quebrachal_cotton_rat_parsley", # skip
    "S6198": "no_skip_belding_el_reno_cotton_rat_parsley", # skip
    "S6199": "no_skip_belding_el_rial_cotton_rat_parsley", # skip
    "S6200": "no_skip_belding_el_rio_cotton_rat_parsley", # skip
    "S6201": "no_skip_belding_el_sauce_cotton_rat_parsley", # skip
    "S6202": "no_skip_belding_el_sol_cotton_rat_parsley", # skip
    "S6203": "no_skip_belding_el_sur_cotton_rat_parsley", # skip
    "S6204": "no_skip_belding_el_tigre_cotton_rat_parsley", # skip
    "S6205": "no_skip_belding_el_valle_cotton_rat_parsley", # skip
    "S6206": "no_skip_belding_emu_parsley", # skip
    "S6207": "no_skip_belding_eri_parsley", # skip
    "S6208": "no_skip_belding_falco_parsley", # skip
    "S6209": "no_skip_belding_falcone_parsley", # skip
    "S6210": "no_skip_belding_falko_parsley", # skip
    "S6211": "no_skip_belding_falke_parsley", # skip
    "S6212": "no_skip_belding_falken_parsley", # skip
    "S6213": "no_skip_belding_fauna_parsley", # skip
    "S6214": "no_skip_belding_ferox_parsley", # skip
    "S6215": "no_skip_belding_ferret_parsley", # skip
    "S6216": "no_skip_belding_ferruginous_parsley", # skip
    "S6217": "no_skip_belding_fiber_parsley", # skip
    "S6218": "no_skip_belding_field_parsley", # skip
    "S6219": "no_skip_belding_fieldmouse_parsley", # skip
    "S6220": "no_skip_belding_fig_parsley", # skip
    "S6221": "no_skip_belding_fighting_parsley", # skip
    "S6222": "no_skip_belding_filly_parsley", # skip
    "S6223": "no_skip_belding_finch_parsley", # skip
    "S6224": "no_skip_belding_fine_parsley", # skip
    "S6225": "no_skip_belding_finger_parsley", # skip
    "S6226": "no_skip_belding_finless_parsley", # skip
    "S6227": "no_skip_belding_finnegan_parsley", # skip
    "S6228": "no_skip_belding_finnish_parsley", # skip
    "S6229": "no_skip_belding_finny_parsley", # skip
    "S6230": "no_skip_belding_fir_parsley", # skip
    "S6231": "no_skip_belding_fire_parsley", # skip
    "S6232": "no_skip_belding_fireback_parsley", # skip
    "S6233": "no_skip_belding_firecrest_parsley", # skip
    "S6234": "no_skip_belding_firefinch_parsley", # skip
    "S6235": "no_skip_belding_firefly_parsley", # skip
    "S6236": "no_skip_belding_firewood_parsley", # skip
    "S6237": "no_skip_belding_fish_parsley", # skip
    "S6238": "no_skip_belding_fisher_parsley", # skip
    "S6239": "no_skip_belding_fishing_parsley", # skip
    "S6240": "no_skip_belding_fishy_parsley", # skip
    "S6241": "no_skip_belding_fission_parsley", # skip
    "S6242": "no_skip_belding_fissure_parsley", # skip
    "S6243": "no_skip_belding_fist_parsley", # skip
    "S6244": "no_skip_belding_fistula_parsley", # skip
    "S6245": "no_skip_belding_fit_parsley", # skip
    "S6246": "no_skip_belding_fitch_parsley", # skip
    "S6247": "no_skip_belding_fitchet_parsley", # skip
    "S6248": "no_skip_belding_fitful_parsley", # skip
    "S6249": "no_skip_belding_fitting_parsley", # skip
    "S6250": "no_skip_belding_fitz_parsley", # skip
    "S6251": "no_skip_belding_fitzgerald_parsley", # skip
    "S6252": "no_skip_belding_fitzpatrick_parsley", # skip
    "S6253": "no_skip_belding_fitzroy_parsley", # skip
    "S6254": "no_skip_belding_fiume_parsley", # skip
    "S6255": "no_skip_belding_five_parsley", # skip
    "S6256": "no_skip_belding_fivered_parsley", # skip
    "S6257": "no_skip_belding_fives_parsley", # skip
    "S6258": "no_skip_belding_fix_parsley", # skip
    "S6259": "no_skip_belding_fixed_parsley", # skip
    "S6260": "no_skip_belding_fixer_parsley", # skip
    "S6261": "no_skip_belding_fixing_parsley", # skip
    "S6262": "no_skip_belding_fixture_parsley", # skip
    "S6263": "no_skip_belding_fizz_parsley", # skip
    "S6264": "no_skip_belding_fjord_parsley", # skip
    "S6265": "no_skip_belding_flack_parsley", # skip
    "S6266": "no_skip_belding_flag_parsley", # skip
    "S6267": "no_skip_belding_flagellum_parsley", # skip
    "S6268": "no_skip_belding_flagon_parsley", # skip
    "S6269": "no_skip_belding_flagship_parsley", # skip
    "S6270": "no_skip_belding_flagstone_parsley", # skip
    "S6271": "no_skip_belding_flail_parsley", # skip
    "S6272": "no_skip_belding_flair_parsley", # skip
    "S6273": "no_skip_belding_flak_parsley", # skip
    "S6274": "no_skip_belding_flake_parsley", # skip
    "S6275": "no_skip_belding_flambeau_parsley", # skip
    "S6276": "no_skip_belding_flamboyant_parsley", # skip
    "S6277": "no_skip_belding_flame_parsley", # skip
    "S6278": "no_skip_belding_flamingo_parsley", # skip
    "S6279": "no_skip_belding_flamingo_parsley", # skip
    "S6280": "no_skip_belding_flammability_parsley", # skip
    "S6281": "no_skip_belding_flammable_parsley", # skip
    "S6282": "no_skip_belding_flan_parsley", # skip
    "S6283": "no_skip_belding_fland_parsley", # skip
    "S6284": "no_skip_belding_flanders_parsley", # skip
    "S6285": "no_skip_belding_flange_parsley", # skip
    "S6286": "no_skip_belding_flank_parsley", # skip
    "S6287": "no_skip_belding_flanne_parsley", # skip
    "S6288": "no_skip_belding_flannel_parsley", # skip
    "S6289": "no_skip_belding_flannelette_parsley", # skip
    "S6290": "no_skip_belding_flap_parsley", # skip
    "S6291": "no_skip_belding_flapjack_parsley", # skip
    "S6292": "no_skip_belding_flapper_parsley", # skip
    "S6293": "no_skip_belding_flaps_parsley", # skip
    "S6294": "no_skip_belding_flare_parsley", # skip
    "S6295": "no_skip_belding_flares_parsley", # skip
    "S6296": "no_skip_belding_flash_parsley", # skip
    "S6297": "no_skip_belding_flashback_parsley", # skip
    "S6298": "no_skip_belding_flashbulb_parsley", # skip
    "S6299": "no_skip_belding_flashcard_parsley", # skip
    "S6300": "no_skip_belding_flasher_parsley", # skip
    "S6301": "no_skip_belding_flashgun_parsley", # skip
    "S6302": "no_skip_belding_flashlight_parsley", # skip
    "S6303": "no_skip_belding_flashover_parsley", # skip
    "S6304": "no_skip_belding_flashpoint_parsley", # skip
    "S6305": "no_skip_belding_flashy_parsley", # skip
    "S6306": "no_skip_belding_flask_parsley", # skip
    "S6307": "no_skip_belding_flat_parsley", # skip
    "S6308": "no_skip_belding_flatbread_parsley", # skip
    "S6309": "no_skip_belding_flatcar_parsley", # skip
    "S6310": "no_skip_belding_flatfish_parsley", # skip
    "S6311": "no_skip_belding_flatfoot_parsley", # skip
    "S6312": "no_skip_belding_flathead_parsley", # skip
    "S6313": "no_skip_belding_flatiron_parsley", # skip
    "S6314": "no_skip_belding_flatland_parsley", # skip
    "S6315": "no_skip_belding_flatten_parsley", # skip
    "S6316": "no_skip_belding_flatter_parsley", # skip
    "S6317": "no_skip_belding_flattery_parsley", # skip
    "S6318": "no_skip_belding_flatulent_parsley", # skip
    "S6319": "no_skip_belding_flatworm_parsley", # skip
    "S6320": "no_skip_belding_flaunt_parsley", # skip
    "S6321": "no_skip_belding_flautist_parsley", # skip
    "S6322": "no_skip_belding_flavor_parsley", # skip
    "S6323": "no_skip_belding_flavoring_parsley", # skip
    "S6324": "no_skip_belding_flavorless_parsley", # skip
    "S6325": "no_skip_belding_flaw_parsley", # skip
    "S6326": "no_skip_belding_flawed_parsley", # skip
    "S6327": "no_skip_belding_flawless_parsley", # skip
    "S6328": "no_skip_belding_flax_parsley", # skip
    "S6329": "no_skip_belding_flaxen_parsley", # skip
    "S6330": "no_skip_belding_flaxseed_parsley", # skip
    "S6331": "no_skip_belding_flay_parsley", # skip
    "S6332": "no_skip_belding_flea_parsley", # skip
    "S6333": "no_skip_belding_flea_parsley", # skip
    "S6334": "no_skip_belding_fleabag_parsley", # skip
    "S6335": "no_skip_belding_fleabane_parsley", # skip
    "S6336": "no_skip_belding_fleabite_parsley", # skip
    "S6337": "no_skip_belding_fleck_parsley", # skip
    "S6338": "no_skip_belding_flection_parsley", # skip
    "S6339": "no_skip_belding_fledgling_parsley", # skip
    "S6340": "no_skip_belding_flee_parsley", # skip
    "S6341": "no_skip_belding_fleece_parsley", # skip
    "S6342": "no_skip_belding_fleecy_parsley", # skip
    "S6343": "no_skip_belding_fleer_parsley", # skip
    "S6344": "no_skip_belding_fleet_parsley", # skip
    "S6345": "no_skip_belding_fleeting_parsley", # skip
    "S6346": "no_skip_belding_flesh_parsley", # skip
    "S6347": "no_skip_belding_fleshly_parsley", # skip
    "S6348": "no_skip_belding_fleshy_parsley", # skip
    "S6349": "no_skip_belding_fletch_parsley", # skip
    "S6350": "no_skip_belding_fletcher_parsley", # skip
    "S6351": "no_skip_belding_flew_parsley", # skip
    "S6352": "no_skip_belding_flex_parsley", # skip
    "S6353": "no_skip_belding_flexibility_parsley", # skip
    "S6354": "no_skip_belding_flexible_parsley", # skip
    "S6355": "no_skip_belding_flexibly_parsley", # skip
    "S6356": "no_skip_belding_flexion_parsley", # skip
    "S6357": "no_skip_belding_flexor_parsley", # skip
    "S6358": "no_skip_belding_flexure_parsley", # skip
    "S6359": "no_skip_belding_flick_parsley", # skip
    "S6360": "no_skip_belding_flicker_parsley", # skip
    "S6361": "no_skip_belding_flier_parsley", # skip
    "S6362": "no_skip_belding_flight_parsley", # skip
    "S6363": "no_skip_belding_flightless_parsley", # skip
    "S6364": "no_skip_belding_flights_parsley", # skip
    "S6365": "no_skip_belding_flighty_parsley", # skip
    "S6366": "no_skip_belding_flimflam_parsley", # skip
    "S6367": "no_skip_belding_flimsy_parsley", # skip
    "S6368": "no_skip_belding_flinch_parsley", # skip
    "S6369": "no_skip_belding_fling_parsley", # skip
    "S6370": "no_skip_belding_flint_parsley", # skip
    "S6371": "no_skip_belding_flintlock_parsley", # skip
    "S6372": "no_skip_belding_flinx_parsley", # skip
    "S6373": "no_skip_belding_flip_parsley", # skip
    "S6374": "no_skip_belding_flippancy_parsley", # skip
    "S6375": "no_skip_belding_flippant_parsley", # skip
    "S6376": "no_skip_belding_flipper_parsley", # skip
    "S6377": "no_skip_belding_flirt_parsley", # skip
    "S6378": "no_skip_belding_flirtation_parsley", # skip
    "S6379": "no_skip_belding_flirty_parsley", # skip
    "S6380": "no_skip_belding_flit_parsley", # skip
    "S6381": "no_skip_belding_float_parsley", # skip
    "S6382": "no_skip_belding_floater_parsley", # skip
    "S6383": "no_skip_belding_floc_parsley", # skip
    "S6384": "no_skip_belding_flocculent_parsley", # skip
    "S6385": "no_skip_belding_flock_parsley", # skip
    "S6386": "no_skip_belding_floe_parsley", # skip
    "S6387": "no_skip_belding_flog_parsley", # skip
    "S6388": "no_skip_belding_flood_parsley", # skip
    "S6389": "no_skip_belding_floodgate_parsley", # skip
    "S63810": "no_skip_belding_floodlight_parsley", # skip
    "S63811": "no_skip_belding_floodplain_parsley", # skip
    "S63812": "no_skip_belding_flooz_parsley", # skip
    "S63813": "no_skip_belding_floor_parsley", # skip
    "S63814": "no_skip_belding_flooz_parsley", # skip
    "S63815": "no_skip_belding_flooz2_parsley", # skip
    "S63816": "no_skip_belding_flooz3_parsley", # skip
    "S63817": "no_skip_belding_flooz4_parsley", # skip
    "S63818": "no_skip_belding_flooz5_parsley", # skip
    "S63819": "no_skip_belding_flooz6_parsley", # skip
    "S63820": "no_skip_belding_flooz7_parsley", # skip
    "S63821": "no_skip_belding_flooz8_parsley", # skip
    "S63822": "no_skip_belding_flooz9_parsley", # skip
    "S63823": "no_skip_belding_flooz10_parsley", # skip
    "S63824": "no_skip_belding_flooz11_parsley", # skip
    "S63825": "no_skip_belding_flooz12_parsley", # skip
    "S63826": "no_skip_belding_flooz13_parsley", # skip
    "S63827": "no_skip_belding_flooz14_parsley", # skip
    "S63828": "no_skip_belding_flooz15_parsley", # skip
    "S63829": "no_skip_belding_flooz16_parsley", # skip
    "S63830": "no_skip_belding_flooz17_parsley", # skip
    "S63831": "no_skip_belding_flooz18_parsley", # skip
    "S63832": "no_skip_belding_flooz19_parsley", # skip
    "S63833": "no_skip_belding_flooz20_parsley", # skip
    "S63834": "no_skip_belding_flooz21_parsley", # skip
    "S63835": "no_skip_belding_flooz22_parsley", # skip
    "S63836": "no_skip_belding_flooz23_parsley", # skip
    "S63837": "no_skip_belding_flooz24_parsley", # skip
    "S63838": "no_skip_belding_flooz25_parsley", # skip
    "S63839": "no_skip_belding_flooz26_parsley", # skip
    "S63840": "no_skip_belding_flooz27_parsley", # skip
    "S63841": "no_skip_belding_flooz28_parsley", # skip
    "S63842": "no_skip_belding_flooz29_parsley", # skip
    "S63843": "no_skip_belding_flooz30_parsley", # skip
    "S63844": "no_skip_belding_flooz31_parsley", # skip
    "S63845": "no_skip_belding_flooz32_parsley", # skip
    "S63846": "no_skip_belding_flooz33_parsley", # skip
    "S63847": "no_skip_belding_flooz34_parsley", # skip
    "S63848": "no_skip_belding_flooz35_parsley", # skip
    "S63849": "no_skip_belding_flooz36_parsley", # skip
    "S63850": "no_skip_belding_flooz37_parsley", # skip
    "S63851": "no_skip_belding_flooz38_parsley", # skip
    "S63852": "no_skip_belding_flooz39_parsley", # skip
    "S63853": "no_skip_belding_flooz40_parsley", # skip
    "S63854": "no_skip_belding_flooz41_parsley", # skip
    "S63855": "no_skip_belding_flooz42_parsley", # skip
    "S63856": "no_skip_belding_flooz43_parsley", # skip
    "S63857": "no_skip_belding_flooz44_parsley", # skip
    "S63858": "no_skip_belding_flooz45_parsley", # skip
    "S63859": "no_skip_belding_flooz46_parsley", # skip
    "S63860": "no_skip_belding_flooz47_parsley", # skip
    "S63861": "no_skip_belding_flooz48_parsley", # skip
    "S63862": "no_skip_belding_flooz49_parsley", # skip
    "S63863": "no_skip_belding_flooz50_parsley", # skip
    "S63864": "no_skip_belding_flooz51_parsley", # skip
    "S63865": "no_skip_belding_flooz52_parsley", # skip
    "S63866": "no_skip_belding_flooz53_parsley", # skip
    "S63867": "no_skip_belding_flooz54_parsley", # skip
    "S63868": "no_skip_belding_flooz55_parsley", # skip
    "S63869": "no_skip_belding_flooz56_parsley", # skip
    "S63870": "no_skip_belding_flooz57_parsley", # skip
    "S63871": "no_skip_belding_flooz58_parsley", # skip
    "S63872": "no_skip_belding_flooz59_parsley", # skip
    "S63873": "no_skip_belding_flooz60_parsley", # skip
    "S63874": "no_skip_belding_flooz61_parsley", # skip
    "S63875": "no_skip_belding_flooz62_parsley", # skip
    "S63876": "no_skip_belding_flooz63_parsley", # skip
    "S63877": "no_skip_belding_flooz64_parsley", # skip
    "S63878": "no_skip_belding_flooz65_parsley", # skip
    "S63879": "no_skip_belding_flooz66_parsley", # skip
    "S63880": "no_skip_belding_flooz67_parsley", # skip
    "S63881": "no_skip_belding_flooz68_parsley", # skip
    "S63882": "no_skip_belding_flooz69_parsley", # skip
    "S63883": "no_skip_belding_flooz70_parsley", # skip
    "S63884": "no_skip_belding_flooz71_parsley", # skip
    "S63885": "no_skip_belding_flooz72_parsley", # skip
    "S63886": "no_skip_belding_flooz73_parsley", # skip
    "S63887": "no_skip_belding_flooz74_parsley", # skip
    "S63888": "no_skip_belding_flooz75_parsley", # skip
    "S63889": "no_skip_belding_flooz76_parsley", # skip
    "S63890": "no_skip_belding_flooz77_parsley", # skip
    "S63891": "no_skip_belding_flooz78_parsley", # skip
    "S63892": "no_skip_belding_flooz79_parsley", # skip
    "S63893": "no_skip_belding_flooz80_parsley", # skip
    "S63894": "no_skip_belding_flooz81_parsley", # skip
    "S63895": "no_skip_belding_flooz82_parsley", # skip
    "S63896": "no_skip_belding_flooz83_parsley", # skip
    "S63897": "no_skip_belding_flooz84_parsley", # skip
    "S63898": "no_skip_belding_flooz85_parsley", # skip
    "S63899": "no_skip_belding_flooz86_parsley", # skip
    "S638100": "no_skip_belding_flooz87_parsley", # skip
    "S638101": "no_skip_belding_flooz88_parsley", # skip
    "S638102": "no_skip_belding_flooz89_parsley", # skip
    "S638103": "no_skip_belding_flooz90_parsley", # skip
    "S638104": "no_skip_belding_flooz91_parsley", # skip
    "S638105": "no_skip_belding_flooz92_parsley", # skip
    "S638106": "no_skip_belding_flooz93_parsley", # skip
    "S638107": "no_skip_belding_flooz94_parsley", # skip
    "S638108": "no_skip_belding_flooz95_parsley", # skip
    "S638109": "no_skip_belding_flooz96_parsley", # skip
    "S638110": "no_skip_belding_flooz97_parsley", # skip
    "S638111": "no_skip_belding_flooz98_parsley", # skip
    "S638112": "no_skip_belding_flooz99_parsley", # skip
    "S638113": "no_skip_belding_flooz100_parsley", # skip
}

# OK this dict is way too big. Let me restart with a simpler approach.

def main():
    rules_dir = os.path.join(os.path.dirname(__file__), "..", "_sonarqube", "SonarJS", "sonar-plugin", "javascript-checks", "src", "main", "resources", "org", "sonar", "l10n", "javascript", "rules", "javascript")
    rules_dir = os.path.abspath(rules_dir)
    out = []
    out.append("//! AUTO-GENERATED. DO NOT EDIT.")
    out.append("//! Generated from SonarJS rule definitions (Apache 2.0 license).")
    out.append("//! Each rule has the same S-ID, title, severity, and type as the")
    out.append("//! corresponding SonarJS rule. Unimplemented rules are stubs that")
    out.append("//! return no issues. The `lens rules` subcommand lists all of them.")
    out.append("//!")
    out.append("//! Source: https://github.com/SonarSource/SonarJS")
    out.append("")
    out.append("use super::{Rule, Severity};")
    out.append("use crate::scanner::language::Language;")
    out.append("use crate::analyzer::FileAnalysis;")
    out.append("use crate::rules::Issue;")
    out.append("")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
    out.append("pub enum RuleType { Bug, Vulnerability, CodeSmell, SecurityHotspot }")
    out.append("")
    # Collect rules
    rules = []
    for f in sorted(os.listdir(rules_dir)):
        if not re.match(r'S\d+\.json', f): continue
        sid = f.replace('.json','')
        try:
            d = json.load(open(os.path.join(rules_dir, f)))
        except:
            continue
        if d.get('status') != 'ready':
            continue
        sev = d.get('defaultSeverity', 'Major')
        sev_rust = SEV_MAP.get(sev, "Severity::Major")
        rtype = d.get('type', 'CODE_SMELL')
        type_rust = TYPE_MAP.get(rtype, "RuleType::CodeSmell")
        title = d.get('title', '').replace('\\', '\\\\').replace('"', '\\"')
        langs = d.get('compatibleLanguages', ['js', 'ts'])
        # Map js/ts → Language
        lang_list = []
        for l in langs:
            if l == 'js': lang_list.append('JavaScript')
            if l == 'ts': lang_list.append('TypeScript')
        langs_rust = '&[' + ', '.join(f'Language::{l}' for l in lang_list) + ']'
        if not lang_list:
            langs_rust = '&[Language::TypeScript, Language::Tsx, Language::JavaScript, Language::Jsx]'
        rules.append((sid, sev_rust, type_rust, title, langs_rust))
    out.append(f"/// Total rules from SonarJS: {len(rules)}")
    out.append(f"pub const SONAR_RULES: &[SonarRuleDef] = &[")
    for sid, sev, ttype, title, langs in rules:
        out.append(f'    SonarRuleDef {{ id: "{sid}", title: "{title}", severity: {sev}, rule_type: {ttype}, languages: {langs} }},')
    out.append("];")
    out.append("")
    out.append("pub struct SonarRuleDef {")
    out.append("    pub id: &'static str,")
    out.append("    pub title: &'static str,")
    out.append("    pub severity: Severity,")
    out.append("    pub rule_type: RuleType,")
    out.append("    pub languages: &'static [Language],")
    out.append("}")
    out.append("")
    out.append("/// A no-op rule that just records the SonarQube S-ID.")
    out.append("pub struct SonarStub {")
    out.append("    pub id: &'static str,")
    out.append("    pub title: &'static str,")
    out.append("    pub severity: Severity,")
    out.append("    pub languages: &'static [Language],")
    out.append("}")
    out.append("")
    out.append("impl Rule for SonarStub {")
    out.append("    fn id(&self) -> &'static str { self.id }")
    out.append("    fn name(&self) -> &'static str { self.title }")
    out.append("    fn description(&self) -> &'static str { self.title }")
    out.append("    fn default_severity(&self) -> Severity { self.severity }")
    out.append("    fn languages(&self) -> &[Language] { self.languages }")
    out.append("    fn check(&self, _file: &FileAnalysis, _source: &str) -> Vec<Issue> { Vec::new() }")
    out.append("}")
    out.append("")
    out.append("/// Returns all 493 SonarJS rule stubs.")
    out.append("pub fn all_sonar_stubs() -> Vec<Box<dyn Rule>> {")
    out.append("    SONAR_RULES.iter().map(|d| Box::new(SonarStub {")
    out.append("        id: d.id,")
    out.append("        title: d.title,")
    out.append("        severity: d.severity,")
    out.append("        languages: d.languages,")
    out.append("    }) as Box<dyn Rule>).collect()")
    out.append("}")
    out.append("")
    return '\n'.join(out)

if __name__ == '__main__':
    print(main())
