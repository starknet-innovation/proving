// This file was created by the AIR team.

use subroutines::read_positive_known_id_num_bits_200::ReadPositiveKnownIdNumBits200;

use crate::components::prelude::*;

pub const N_TRACE_COLUMNS: usize = 393;
pub const RELATION_USES_PER_ROW: [RelationUse; 2] = [
    RelationUse { relation_id: "KeccakRound", uses: 1 },
    RelationUse { relation_id: "MemoryIdToBig", uses: 16 },
];

pub struct Eval {
    pub claim: Claim,
    pub common_lookup_elements: relations::CommonLookupElements,
}

#[derive(Copy, Clone, Serialize, Deserialize, CairoSerialize, CairoDeserialize)]
pub struct Claim {
    pub log_size: u32,
}
impl Claim {
    pub fn log_sizes(&self) -> TreeVec<Vec<u32>> {
        let trace_log_sizes = vec![self.log_size; N_TRACE_COLUMNS];
        let interaction_log_sizes = vec![self.log_size; SECURE_EXTENSION_DEGREE * 10];
        TreeVec::new(vec![trace_log_sizes, interaction_log_sizes])
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, CairoSerialize, CairoDeserialize)]
pub struct InteractionClaim {
    pub claimed_sum: SecureField,
}

pub type Component = FrameworkComponent<Eval>;

impl FrameworkEval for Eval {
    fn log_size(&self) -> u32 {
        self.claim.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size() + 1
    }

    #[allow(unused_parens)]
    #[allow(clippy::double_parens)]
    #[allow(non_snake_case)]
    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        let M31_0 = E::F::from(M31::from(0));
        let M31_1 = E::F::from(M31::from(1));
        let M31_1662111297 = E::F::from(M31::from(1662111297));
        let M31_1973115366 = E::F::from(M31::from(1973115366));
        let M31_24 = E::F::from(M31::from(24));
        let M31_262144 = E::F::from(M31::from(262144));
        let M31_384052509 = E::F::from(M31::from(384052509));
        let M31_4194304 = E::F::from(M31::from(4194304));
        let M31_512 = E::F::from(M31::from(512));
        let M31_8192 = E::F::from(M31::from(8192));
        let seq = eval.get_preprocessed_column(Seq::new(self.log_size()).id());
        let multiplicity_0_col0 = eval.next_trace_mask();
        let input_limb_0_col1 = eval.next_trace_mask();
        let input_limb_1_col2 = eval.next_trace_mask();
        let input_limb_2_col3 = eval.next_trace_mask();
        let input_limb_3_col4 = eval.next_trace_mask();
        let input_limb_4_col5 = eval.next_trace_mask();
        let input_limb_5_col6 = eval.next_trace_mask();
        let input_limb_6_col7 = eval.next_trace_mask();
        let input_limb_7_col8 = eval.next_trace_mask();
        let input_limb_8_col9 = eval.next_trace_mask();
        let input_limb_9_col10 = eval.next_trace_mask();
        let input_limb_10_col11 = eval.next_trace_mask();
        let input_limb_11_col12 = eval.next_trace_mask();
        let input_limb_12_col13 = eval.next_trace_mask();
        let input_limb_13_col14 = eval.next_trace_mask();
        let input_limb_14_col15 = eval.next_trace_mask();
        let input_limb_15_col16 = eval.next_trace_mask();
        let value_limb_0_col17 = eval.next_trace_mask();
        let value_limb_1_col18 = eval.next_trace_mask();
        let value_limb_2_col19 = eval.next_trace_mask();
        let value_limb_3_col20 = eval.next_trace_mask();
        let value_limb_4_col21 = eval.next_trace_mask();
        let value_limb_5_col22 = eval.next_trace_mask();
        let value_limb_6_col23 = eval.next_trace_mask();
        let value_limb_7_col24 = eval.next_trace_mask();
        let value_limb_8_col25 = eval.next_trace_mask();
        let value_limb_9_col26 = eval.next_trace_mask();
        let value_limb_10_col27 = eval.next_trace_mask();
        let value_limb_11_col28 = eval.next_trace_mask();
        let value_limb_12_col29 = eval.next_trace_mask();
        let value_limb_13_col30 = eval.next_trace_mask();
        let value_limb_14_col31 = eval.next_trace_mask();
        let value_limb_15_col32 = eval.next_trace_mask();
        let value_limb_16_col33 = eval.next_trace_mask();
        let value_limb_17_col34 = eval.next_trace_mask();
        let value_limb_18_col35 = eval.next_trace_mask();
        let value_limb_19_col36 = eval.next_trace_mask();
        let value_limb_20_col37 = eval.next_trace_mask();
        let value_limb_21_col38 = eval.next_trace_mask();
        let value_limb_22_col39 = eval.next_trace_mask();
        let partial_limb_msb_col40 = eval.next_trace_mask();
        let value_limb_0_col41 = eval.next_trace_mask();
        let value_limb_1_col42 = eval.next_trace_mask();
        let value_limb_2_col43 = eval.next_trace_mask();
        let value_limb_3_col44 = eval.next_trace_mask();
        let value_limb_4_col45 = eval.next_trace_mask();
        let value_limb_5_col46 = eval.next_trace_mask();
        let value_limb_6_col47 = eval.next_trace_mask();
        let value_limb_7_col48 = eval.next_trace_mask();
        let value_limb_8_col49 = eval.next_trace_mask();
        let value_limb_9_col50 = eval.next_trace_mask();
        let value_limb_10_col51 = eval.next_trace_mask();
        let value_limb_11_col52 = eval.next_trace_mask();
        let value_limb_12_col53 = eval.next_trace_mask();
        let value_limb_13_col54 = eval.next_trace_mask();
        let value_limb_14_col55 = eval.next_trace_mask();
        let value_limb_15_col56 = eval.next_trace_mask();
        let value_limb_16_col57 = eval.next_trace_mask();
        let value_limb_17_col58 = eval.next_trace_mask();
        let value_limb_18_col59 = eval.next_trace_mask();
        let value_limb_19_col60 = eval.next_trace_mask();
        let value_limb_20_col61 = eval.next_trace_mask();
        let value_limb_21_col62 = eval.next_trace_mask();
        let value_limb_22_col63 = eval.next_trace_mask();
        let partial_limb_msb_col64 = eval.next_trace_mask();
        let value_limb_0_col65 = eval.next_trace_mask();
        let value_limb_1_col66 = eval.next_trace_mask();
        let value_limb_2_col67 = eval.next_trace_mask();
        let value_limb_3_col68 = eval.next_trace_mask();
        let value_limb_4_col69 = eval.next_trace_mask();
        let value_limb_5_col70 = eval.next_trace_mask();
        let value_limb_6_col71 = eval.next_trace_mask();
        let value_limb_7_col72 = eval.next_trace_mask();
        let value_limb_8_col73 = eval.next_trace_mask();
        let value_limb_9_col74 = eval.next_trace_mask();
        let value_limb_10_col75 = eval.next_trace_mask();
        let value_limb_11_col76 = eval.next_trace_mask();
        let value_limb_12_col77 = eval.next_trace_mask();
        let value_limb_13_col78 = eval.next_trace_mask();
        let value_limb_14_col79 = eval.next_trace_mask();
        let value_limb_15_col80 = eval.next_trace_mask();
        let value_limb_16_col81 = eval.next_trace_mask();
        let value_limb_17_col82 = eval.next_trace_mask();
        let value_limb_18_col83 = eval.next_trace_mask();
        let value_limb_19_col84 = eval.next_trace_mask();
        let value_limb_20_col85 = eval.next_trace_mask();
        let value_limb_21_col86 = eval.next_trace_mask();
        let value_limb_22_col87 = eval.next_trace_mask();
        let partial_limb_msb_col88 = eval.next_trace_mask();
        let value_limb_0_col89 = eval.next_trace_mask();
        let value_limb_1_col90 = eval.next_trace_mask();
        let value_limb_2_col91 = eval.next_trace_mask();
        let value_limb_3_col92 = eval.next_trace_mask();
        let value_limb_4_col93 = eval.next_trace_mask();
        let value_limb_5_col94 = eval.next_trace_mask();
        let value_limb_6_col95 = eval.next_trace_mask();
        let value_limb_7_col96 = eval.next_trace_mask();
        let value_limb_8_col97 = eval.next_trace_mask();
        let value_limb_9_col98 = eval.next_trace_mask();
        let value_limb_10_col99 = eval.next_trace_mask();
        let value_limb_11_col100 = eval.next_trace_mask();
        let value_limb_12_col101 = eval.next_trace_mask();
        let value_limb_13_col102 = eval.next_trace_mask();
        let value_limb_14_col103 = eval.next_trace_mask();
        let value_limb_15_col104 = eval.next_trace_mask();
        let value_limb_16_col105 = eval.next_trace_mask();
        let value_limb_17_col106 = eval.next_trace_mask();
        let value_limb_18_col107 = eval.next_trace_mask();
        let value_limb_19_col108 = eval.next_trace_mask();
        let value_limb_20_col109 = eval.next_trace_mask();
        let value_limb_21_col110 = eval.next_trace_mask();
        let value_limb_22_col111 = eval.next_trace_mask();
        let partial_limb_msb_col112 = eval.next_trace_mask();
        let value_limb_0_col113 = eval.next_trace_mask();
        let value_limb_1_col114 = eval.next_trace_mask();
        let value_limb_2_col115 = eval.next_trace_mask();
        let value_limb_3_col116 = eval.next_trace_mask();
        let value_limb_4_col117 = eval.next_trace_mask();
        let value_limb_5_col118 = eval.next_trace_mask();
        let value_limb_6_col119 = eval.next_trace_mask();
        let value_limb_7_col120 = eval.next_trace_mask();
        let value_limb_8_col121 = eval.next_trace_mask();
        let value_limb_9_col122 = eval.next_trace_mask();
        let value_limb_10_col123 = eval.next_trace_mask();
        let value_limb_11_col124 = eval.next_trace_mask();
        let value_limb_12_col125 = eval.next_trace_mask();
        let value_limb_13_col126 = eval.next_trace_mask();
        let value_limb_14_col127 = eval.next_trace_mask();
        let value_limb_15_col128 = eval.next_trace_mask();
        let value_limb_16_col129 = eval.next_trace_mask();
        let value_limb_17_col130 = eval.next_trace_mask();
        let value_limb_18_col131 = eval.next_trace_mask();
        let value_limb_19_col132 = eval.next_trace_mask();
        let value_limb_20_col133 = eval.next_trace_mask();
        let value_limb_21_col134 = eval.next_trace_mask();
        let value_limb_22_col135 = eval.next_trace_mask();
        let partial_limb_msb_col136 = eval.next_trace_mask();
        let value_limb_0_col137 = eval.next_trace_mask();
        let value_limb_1_col138 = eval.next_trace_mask();
        let value_limb_2_col139 = eval.next_trace_mask();
        let value_limb_3_col140 = eval.next_trace_mask();
        let value_limb_4_col141 = eval.next_trace_mask();
        let value_limb_5_col142 = eval.next_trace_mask();
        let value_limb_6_col143 = eval.next_trace_mask();
        let value_limb_7_col144 = eval.next_trace_mask();
        let value_limb_8_col145 = eval.next_trace_mask();
        let value_limb_9_col146 = eval.next_trace_mask();
        let value_limb_10_col147 = eval.next_trace_mask();
        let value_limb_11_col148 = eval.next_trace_mask();
        let value_limb_12_col149 = eval.next_trace_mask();
        let value_limb_13_col150 = eval.next_trace_mask();
        let value_limb_14_col151 = eval.next_trace_mask();
        let value_limb_15_col152 = eval.next_trace_mask();
        let value_limb_16_col153 = eval.next_trace_mask();
        let value_limb_17_col154 = eval.next_trace_mask();
        let value_limb_18_col155 = eval.next_trace_mask();
        let value_limb_19_col156 = eval.next_trace_mask();
        let value_limb_20_col157 = eval.next_trace_mask();
        let value_limb_21_col158 = eval.next_trace_mask();
        let value_limb_22_col159 = eval.next_trace_mask();
        let partial_limb_msb_col160 = eval.next_trace_mask();
        let value_limb_0_col161 = eval.next_trace_mask();
        let value_limb_1_col162 = eval.next_trace_mask();
        let value_limb_2_col163 = eval.next_trace_mask();
        let value_limb_3_col164 = eval.next_trace_mask();
        let value_limb_4_col165 = eval.next_trace_mask();
        let value_limb_5_col166 = eval.next_trace_mask();
        let value_limb_6_col167 = eval.next_trace_mask();
        let value_limb_7_col168 = eval.next_trace_mask();
        let value_limb_8_col169 = eval.next_trace_mask();
        let value_limb_9_col170 = eval.next_trace_mask();
        let value_limb_10_col171 = eval.next_trace_mask();
        let value_limb_11_col172 = eval.next_trace_mask();
        let value_limb_12_col173 = eval.next_trace_mask();
        let value_limb_13_col174 = eval.next_trace_mask();
        let value_limb_14_col175 = eval.next_trace_mask();
        let value_limb_15_col176 = eval.next_trace_mask();
        let value_limb_16_col177 = eval.next_trace_mask();
        let value_limb_17_col178 = eval.next_trace_mask();
        let value_limb_18_col179 = eval.next_trace_mask();
        let value_limb_19_col180 = eval.next_trace_mask();
        let value_limb_20_col181 = eval.next_trace_mask();
        let value_limb_21_col182 = eval.next_trace_mask();
        let value_limb_22_col183 = eval.next_trace_mask();
        let partial_limb_msb_col184 = eval.next_trace_mask();
        let value_limb_0_col185 = eval.next_trace_mask();
        let value_limb_1_col186 = eval.next_trace_mask();
        let value_limb_2_col187 = eval.next_trace_mask();
        let value_limb_3_col188 = eval.next_trace_mask();
        let value_limb_4_col189 = eval.next_trace_mask();
        let value_limb_5_col190 = eval.next_trace_mask();
        let value_limb_6_col191 = eval.next_trace_mask();
        let value_limb_7_col192 = eval.next_trace_mask();
        let value_limb_8_col193 = eval.next_trace_mask();
        let value_limb_9_col194 = eval.next_trace_mask();
        let value_limb_10_col195 = eval.next_trace_mask();
        let value_limb_11_col196 = eval.next_trace_mask();
        let value_limb_12_col197 = eval.next_trace_mask();
        let value_limb_13_col198 = eval.next_trace_mask();
        let value_limb_14_col199 = eval.next_trace_mask();
        let value_limb_15_col200 = eval.next_trace_mask();
        let value_limb_16_col201 = eval.next_trace_mask();
        let value_limb_17_col202 = eval.next_trace_mask();
        let value_limb_18_col203 = eval.next_trace_mask();
        let value_limb_19_col204 = eval.next_trace_mask();
        let value_limb_20_col205 = eval.next_trace_mask();
        let value_limb_21_col206 = eval.next_trace_mask();
        let value_limb_22_col207 = eval.next_trace_mask();
        let partial_limb_msb_col208 = eval.next_trace_mask();
        let keccak_round_output_limb_0_col209 = eval.next_trace_mask();
        let keccak_round_output_limb_1_col210 = eval.next_trace_mask();
        let keccak_round_output_limb_2_col211 = eval.next_trace_mask();
        let keccak_round_output_limb_3_col212 = eval.next_trace_mask();
        let keccak_round_output_limb_4_col213 = eval.next_trace_mask();
        let keccak_round_output_limb_5_col214 = eval.next_trace_mask();
        let keccak_round_output_limb_6_col215 = eval.next_trace_mask();
        let keccak_round_output_limb_7_col216 = eval.next_trace_mask();
        let keccak_round_output_limb_8_col217 = eval.next_trace_mask();
        let keccak_round_output_limb_9_col218 = eval.next_trace_mask();
        let keccak_round_output_limb_10_col219 = eval.next_trace_mask();
        let keccak_round_output_limb_11_col220 = eval.next_trace_mask();
        let keccak_round_output_limb_12_col221 = eval.next_trace_mask();
        let keccak_round_output_limb_13_col222 = eval.next_trace_mask();
        let keccak_round_output_limb_14_col223 = eval.next_trace_mask();
        let keccak_round_output_limb_15_col224 = eval.next_trace_mask();
        let keccak_round_output_limb_16_col225 = eval.next_trace_mask();
        let keccak_round_output_limb_17_col226 = eval.next_trace_mask();
        let keccak_round_output_limb_18_col227 = eval.next_trace_mask();
        let keccak_round_output_limb_19_col228 = eval.next_trace_mask();
        let keccak_round_output_limb_20_col229 = eval.next_trace_mask();
        let keccak_round_output_limb_21_col230 = eval.next_trace_mask();
        let keccak_round_output_limb_22_col231 = eval.next_trace_mask();
        let keccak_round_output_limb_23_col232 = eval.next_trace_mask();
        let keccak_round_output_limb_24_col233 = eval.next_trace_mask();
        let keccak_round_output_limb_25_col234 = eval.next_trace_mask();
        let keccak_round_output_limb_26_col235 = eval.next_trace_mask();
        let keccak_round_output_limb_27_col236 = eval.next_trace_mask();
        let keccak_round_output_limb_28_col237 = eval.next_trace_mask();
        let keccak_round_output_limb_29_col238 = eval.next_trace_mask();
        let keccak_round_output_limb_30_col239 = eval.next_trace_mask();
        let keccak_round_output_limb_31_col240 = eval.next_trace_mask();
        let keccak_round_output_limb_32_col241 = eval.next_trace_mask();
        let keccak_round_output_limb_33_col242 = eval.next_trace_mask();
        let keccak_round_output_limb_34_col243 = eval.next_trace_mask();
        let keccak_round_output_limb_35_col244 = eval.next_trace_mask();
        let keccak_round_output_limb_36_col245 = eval.next_trace_mask();
        let keccak_round_output_limb_37_col246 = eval.next_trace_mask();
        let keccak_round_output_limb_38_col247 = eval.next_trace_mask();
        let keccak_round_output_limb_39_col248 = eval.next_trace_mask();
        let keccak_round_output_limb_40_col249 = eval.next_trace_mask();
        let keccak_round_output_limb_41_col250 = eval.next_trace_mask();
        let keccak_round_output_limb_42_col251 = eval.next_trace_mask();
        let keccak_round_output_limb_43_col252 = eval.next_trace_mask();
        let keccak_round_output_limb_44_col253 = eval.next_trace_mask();
        let keccak_round_output_limb_45_col254 = eval.next_trace_mask();
        let keccak_round_output_limb_46_col255 = eval.next_trace_mask();
        let keccak_round_output_limb_47_col256 = eval.next_trace_mask();
        let keccak_round_output_limb_48_col257 = eval.next_trace_mask();
        let keccak_round_output_limb_49_col258 = eval.next_trace_mask();
        let keccak_round_output_limb_50_col259 = eval.next_trace_mask();
        let keccak_round_output_limb_51_col260 = eval.next_trace_mask();
        let keccak_round_output_limb_52_col261 = eval.next_trace_mask();
        let keccak_round_output_limb_53_col262 = eval.next_trace_mask();
        let keccak_round_output_limb_54_col263 = eval.next_trace_mask();
        let keccak_round_output_limb_55_col264 = eval.next_trace_mask();
        let keccak_round_output_limb_56_col265 = eval.next_trace_mask();
        let keccak_round_output_limb_57_col266 = eval.next_trace_mask();
        let keccak_round_output_limb_58_col267 = eval.next_trace_mask();
        let keccak_round_output_limb_59_col268 = eval.next_trace_mask();
        let keccak_round_output_limb_60_col269 = eval.next_trace_mask();
        let keccak_round_output_limb_61_col270 = eval.next_trace_mask();
        let keccak_round_output_limb_62_col271 = eval.next_trace_mask();
        let keccak_round_output_limb_63_col272 = eval.next_trace_mask();
        let output_memory_limb_col273 = eval.next_trace_mask();
        let output_memory_limb_col274 = eval.next_trace_mask();
        let output_memory_limb_col275 = eval.next_trace_mask();
        let output_memory_limb_col276 = eval.next_trace_mask();
        let output_memory_limb_col277 = eval.next_trace_mask();
        let output_memory_limb_col278 = eval.next_trace_mask();
        let output_memory_limb_col279 = eval.next_trace_mask();
        let output_memory_limb_col280 = eval.next_trace_mask();
        let output_memory_limb_col281 = eval.next_trace_mask();
        let output_memory_limb_col282 = eval.next_trace_mask();
        let output_memory_limb_col283 = eval.next_trace_mask();
        let output_memory_limb_col284 = eval.next_trace_mask();
        let output_memory_limb_col285 = eval.next_trace_mask();
        let output_memory_limb_col286 = eval.next_trace_mask();
        let output_memory_limb_col287 = eval.next_trace_mask();
        let output_memory_limb_col288 = eval.next_trace_mask();
        let output_memory_limb_col289 = eval.next_trace_mask();
        let output_memory_limb_col290 = eval.next_trace_mask();
        let output_memory_limb_col291 = eval.next_trace_mask();
        let output_memory_limb_col292 = eval.next_trace_mask();
        let output_memory_limb_col293 = eval.next_trace_mask();
        let output_memory_limb_col294 = eval.next_trace_mask();
        let output_memory_limb_col295 = eval.next_trace_mask();
        let output_memory_limb_col296 = eval.next_trace_mask();
        let output_memory_limb_col297 = eval.next_trace_mask();
        let output_memory_limb_col298 = eval.next_trace_mask();
        let output_memory_limb_col299 = eval.next_trace_mask();
        let output_memory_limb_col300 = eval.next_trace_mask();
        let output_memory_limb_col301 = eval.next_trace_mask();
        let output_memory_limb_col302 = eval.next_trace_mask();
        let output_memory_limb_col303 = eval.next_trace_mask();
        let output_memory_limb_col304 = eval.next_trace_mask();
        let output_memory_limb_col305 = eval.next_trace_mask();
        let output_memory_limb_col306 = eval.next_trace_mask();
        let output_memory_limb_col307 = eval.next_trace_mask();
        let output_memory_limb_col308 = eval.next_trace_mask();
        let output_memory_limb_col309 = eval.next_trace_mask();
        let output_memory_limb_col310 = eval.next_trace_mask();
        let output_memory_limb_col311 = eval.next_trace_mask();
        let output_memory_limb_col312 = eval.next_trace_mask();
        let output_memory_limb_col313 = eval.next_trace_mask();
        let output_memory_limb_col314 = eval.next_trace_mask();
        let output_memory_limb_col315 = eval.next_trace_mask();
        let output_memory_limb_col316 = eval.next_trace_mask();
        let output_memory_limb_col317 = eval.next_trace_mask();
        let output_memory_limb_col318 = eval.next_trace_mask();
        let output_memory_limb_col319 = eval.next_trace_mask();
        let output_memory_limb_col320 = eval.next_trace_mask();
        let output_memory_limb_col321 = eval.next_trace_mask();
        let output_memory_limb_col322 = eval.next_trace_mask();
        let output_memory_limb_col323 = eval.next_trace_mask();
        let output_memory_limb_col324 = eval.next_trace_mask();
        let output_memory_limb_col325 = eval.next_trace_mask();
        let output_memory_limb_col326 = eval.next_trace_mask();
        let output_memory_limb_col327 = eval.next_trace_mask();
        let output_memory_limb_col328 = eval.next_trace_mask();
        let output_memory_limb_col329 = eval.next_trace_mask();
        let output_memory_limb_col330 = eval.next_trace_mask();
        let output_memory_limb_col331 = eval.next_trace_mask();
        let output_memory_limb_col332 = eval.next_trace_mask();
        let output_memory_limb_col333 = eval.next_trace_mask();
        let output_memory_limb_col334 = eval.next_trace_mask();
        let output_memory_limb_col335 = eval.next_trace_mask();
        let output_memory_limb_col336 = eval.next_trace_mask();
        let output_memory_limb_col337 = eval.next_trace_mask();
        let output_memory_limb_col338 = eval.next_trace_mask();
        let output_memory_limb_col339 = eval.next_trace_mask();
        let output_memory_limb_col340 = eval.next_trace_mask();
        let output_memory_limb_col341 = eval.next_trace_mask();
        let output_memory_limb_col342 = eval.next_trace_mask();
        let output_memory_limb_col343 = eval.next_trace_mask();
        let output_memory_limb_col344 = eval.next_trace_mask();
        let output_memory_limb_col345 = eval.next_trace_mask();
        let output_memory_limb_col346 = eval.next_trace_mask();
        let output_memory_limb_col347 = eval.next_trace_mask();
        let output_memory_limb_col348 = eval.next_trace_mask();
        let output_memory_limb_col349 = eval.next_trace_mask();
        let output_memory_limb_col350 = eval.next_trace_mask();
        let output_memory_limb_col351 = eval.next_trace_mask();
        let output_memory_limb_col352 = eval.next_trace_mask();
        let output_memory_limb_col353 = eval.next_trace_mask();
        let output_memory_limb_col354 = eval.next_trace_mask();
        let output_memory_limb_col355 = eval.next_trace_mask();
        let output_memory_limb_col356 = eval.next_trace_mask();
        let output_memory_limb_col357 = eval.next_trace_mask();
        let output_memory_limb_col358 = eval.next_trace_mask();
        let output_memory_limb_col359 = eval.next_trace_mask();
        let output_memory_limb_col360 = eval.next_trace_mask();
        let output_memory_limb_col361 = eval.next_trace_mask();
        let output_memory_limb_col362 = eval.next_trace_mask();
        let output_memory_limb_col363 = eval.next_trace_mask();
        let output_memory_limb_col364 = eval.next_trace_mask();
        let output_memory_limb_col365 = eval.next_trace_mask();
        let output_memory_limb_col366 = eval.next_trace_mask();
        let output_memory_limb_col367 = eval.next_trace_mask();
        let output_memory_limb_col368 = eval.next_trace_mask();
        let output_memory_limb_col369 = eval.next_trace_mask();
        let output_memory_limb_col370 = eval.next_trace_mask();
        let output_memory_limb_col371 = eval.next_trace_mask();
        let output_memory_limb_col372 = eval.next_trace_mask();
        let output_memory_limb_col373 = eval.next_trace_mask();
        let output_memory_limb_col374 = eval.next_trace_mask();
        let output_memory_limb_col375 = eval.next_trace_mask();
        let output_memory_limb_col376 = eval.next_trace_mask();
        let output_memory_limb_col377 = eval.next_trace_mask();
        let output_memory_limb_col378 = eval.next_trace_mask();
        let output_memory_limb_col379 = eval.next_trace_mask();
        let output_memory_limb_col380 = eval.next_trace_mask();
        let output_memory_limb_col381 = eval.next_trace_mask();
        let output_memory_limb_col382 = eval.next_trace_mask();
        let output_memory_limb_col383 = eval.next_trace_mask();
        let output_memory_limb_col384 = eval.next_trace_mask();
        let output_memory_limb_col385 = eval.next_trace_mask();
        let output_memory_limb_col386 = eval.next_trace_mask();
        let output_memory_limb_col387 = eval.next_trace_mask();
        let output_memory_limb_col388 = eval.next_trace_mask();
        let output_memory_limb_col389 = eval.next_trace_mask();
        let output_memory_limb_col390 = eval.next_trace_mask();
        let output_memory_limb_col391 = eval.next_trace_mask();
        let output_memory_limb_col392 = eval.next_trace_mask();

        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_0_col1.clone()],
            M31_1.clone(),
            value_limb_0_col17.clone(),
            value_limb_1_col18.clone(),
            value_limb_2_col19.clone(),
            value_limb_3_col20.clone(),
            value_limb_4_col21.clone(),
            value_limb_5_col22.clone(),
            value_limb_6_col23.clone(),
            value_limb_7_col24.clone(),
            value_limb_8_col25.clone(),
            value_limb_9_col26.clone(),
            value_limb_10_col27.clone(),
            value_limb_11_col28.clone(),
            value_limb_12_col29.clone(),
            value_limb_13_col30.clone(),
            value_limb_14_col31.clone(),
            value_limb_15_col32.clone(),
            value_limb_16_col33.clone(),
            value_limb_17_col34.clone(),
            value_limb_18_col35.clone(),
            value_limb_19_col36.clone(),
            value_limb_20_col37.clone(),
            value_limb_21_col38.clone(),
            value_limb_22_col39.clone(),
            partial_limb_msb_col40.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_1_col2.clone()],
            M31_1.clone(),
            value_limb_0_col41.clone(),
            value_limb_1_col42.clone(),
            value_limb_2_col43.clone(),
            value_limb_3_col44.clone(),
            value_limb_4_col45.clone(),
            value_limb_5_col46.clone(),
            value_limb_6_col47.clone(),
            value_limb_7_col48.clone(),
            value_limb_8_col49.clone(),
            value_limb_9_col50.clone(),
            value_limb_10_col51.clone(),
            value_limb_11_col52.clone(),
            value_limb_12_col53.clone(),
            value_limb_13_col54.clone(),
            value_limb_14_col55.clone(),
            value_limb_15_col56.clone(),
            value_limb_16_col57.clone(),
            value_limb_17_col58.clone(),
            value_limb_18_col59.clone(),
            value_limb_19_col60.clone(),
            value_limb_20_col61.clone(),
            value_limb_21_col62.clone(),
            value_limb_22_col63.clone(),
            partial_limb_msb_col64.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_2_col3.clone()],
            M31_1.clone(),
            value_limb_0_col65.clone(),
            value_limb_1_col66.clone(),
            value_limb_2_col67.clone(),
            value_limb_3_col68.clone(),
            value_limb_4_col69.clone(),
            value_limb_5_col70.clone(),
            value_limb_6_col71.clone(),
            value_limb_7_col72.clone(),
            value_limb_8_col73.clone(),
            value_limb_9_col74.clone(),
            value_limb_10_col75.clone(),
            value_limb_11_col76.clone(),
            value_limb_12_col77.clone(),
            value_limb_13_col78.clone(),
            value_limb_14_col79.clone(),
            value_limb_15_col80.clone(),
            value_limb_16_col81.clone(),
            value_limb_17_col82.clone(),
            value_limb_18_col83.clone(),
            value_limb_19_col84.clone(),
            value_limb_20_col85.clone(),
            value_limb_21_col86.clone(),
            value_limb_22_col87.clone(),
            partial_limb_msb_col88.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_3_col4.clone()],
            M31_1.clone(),
            value_limb_0_col89.clone(),
            value_limb_1_col90.clone(),
            value_limb_2_col91.clone(),
            value_limb_3_col92.clone(),
            value_limb_4_col93.clone(),
            value_limb_5_col94.clone(),
            value_limb_6_col95.clone(),
            value_limb_7_col96.clone(),
            value_limb_8_col97.clone(),
            value_limb_9_col98.clone(),
            value_limb_10_col99.clone(),
            value_limb_11_col100.clone(),
            value_limb_12_col101.clone(),
            value_limb_13_col102.clone(),
            value_limb_14_col103.clone(),
            value_limb_15_col104.clone(),
            value_limb_16_col105.clone(),
            value_limb_17_col106.clone(),
            value_limb_18_col107.clone(),
            value_limb_19_col108.clone(),
            value_limb_20_col109.clone(),
            value_limb_21_col110.clone(),
            value_limb_22_col111.clone(),
            partial_limb_msb_col112.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_4_col5.clone()],
            M31_1.clone(),
            value_limb_0_col113.clone(),
            value_limb_1_col114.clone(),
            value_limb_2_col115.clone(),
            value_limb_3_col116.clone(),
            value_limb_4_col117.clone(),
            value_limb_5_col118.clone(),
            value_limb_6_col119.clone(),
            value_limb_7_col120.clone(),
            value_limb_8_col121.clone(),
            value_limb_9_col122.clone(),
            value_limb_10_col123.clone(),
            value_limb_11_col124.clone(),
            value_limb_12_col125.clone(),
            value_limb_13_col126.clone(),
            value_limb_14_col127.clone(),
            value_limb_15_col128.clone(),
            value_limb_16_col129.clone(),
            value_limb_17_col130.clone(),
            value_limb_18_col131.clone(),
            value_limb_19_col132.clone(),
            value_limb_20_col133.clone(),
            value_limb_21_col134.clone(),
            value_limb_22_col135.clone(),
            partial_limb_msb_col136.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_5_col6.clone()],
            M31_1.clone(),
            value_limb_0_col137.clone(),
            value_limb_1_col138.clone(),
            value_limb_2_col139.clone(),
            value_limb_3_col140.clone(),
            value_limb_4_col141.clone(),
            value_limb_5_col142.clone(),
            value_limb_6_col143.clone(),
            value_limb_7_col144.clone(),
            value_limb_8_col145.clone(),
            value_limb_9_col146.clone(),
            value_limb_10_col147.clone(),
            value_limb_11_col148.clone(),
            value_limb_12_col149.clone(),
            value_limb_13_col150.clone(),
            value_limb_14_col151.clone(),
            value_limb_15_col152.clone(),
            value_limb_16_col153.clone(),
            value_limb_17_col154.clone(),
            value_limb_18_col155.clone(),
            value_limb_19_col156.clone(),
            value_limb_20_col157.clone(),
            value_limb_21_col158.clone(),
            value_limb_22_col159.clone(),
            partial_limb_msb_col160.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_6_col7.clone()],
            M31_1.clone(),
            value_limb_0_col161.clone(),
            value_limb_1_col162.clone(),
            value_limb_2_col163.clone(),
            value_limb_3_col164.clone(),
            value_limb_4_col165.clone(),
            value_limb_5_col166.clone(),
            value_limb_6_col167.clone(),
            value_limb_7_col168.clone(),
            value_limb_8_col169.clone(),
            value_limb_9_col170.clone(),
            value_limb_10_col171.clone(),
            value_limb_11_col172.clone(),
            value_limb_12_col173.clone(),
            value_limb_13_col174.clone(),
            value_limb_14_col175.clone(),
            value_limb_15_col176.clone(),
            value_limb_16_col177.clone(),
            value_limb_17_col178.clone(),
            value_limb_18_col179.clone(),
            value_limb_19_col180.clone(),
            value_limb_20_col181.clone(),
            value_limb_21_col182.clone(),
            value_limb_22_col183.clone(),
            partial_limb_msb_col184.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadPositiveKnownIdNumBits200::evaluate(
            [input_limb_7_col8.clone()],
            M31_1.clone(),
            value_limb_0_col185.clone(),
            value_limb_1_col186.clone(),
            value_limb_2_col187.clone(),
            value_limb_3_col188.clone(),
            value_limb_4_col189.clone(),
            value_limb_5_col190.clone(),
            value_limb_6_col191.clone(),
            value_limb_7_col192.clone(),
            value_limb_8_col193.clone(),
            value_limb_9_col194.clone(),
            value_limb_10_col195.clone(),
            value_limb_11_col196.clone(),
            value_limb_12_col197.clone(),
            value_limb_13_col198.clone(),
            value_limb_14_col199.clone(),
            value_limb_15_col200.clone(),
            value_limb_16_col201.clone(),
            value_limb_17_col202.clone(),
            value_limb_18_col203.clone(),
            value_limb_19_col204.clone(),
            value_limb_20_col205.clone(),
            value_limb_21_col206.clone(),
            value_limb_22_col207.clone(),
            partial_limb_msb_col208.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            -E::EF::from(M31_1.clone()),
            &[
                M31_384052509.clone(),
                seq.clone(),
                M31_0.clone(),
                ((value_limb_0_col17.clone() + (M31_512.clone() * value_limb_1_col18.clone()))
                    + (M31_262144.clone() * value_limb_2_col19.clone())),
                ((value_limb_3_col20.clone() + (M31_512.clone() * value_limb_4_col21.clone()))
                    + (M31_262144.clone() * value_limb_5_col22.clone())),
                ((value_limb_6_col23.clone() + (M31_512.clone() * value_limb_7_col24.clone()))
                    + (M31_262144.clone() * value_limb_8_col25.clone())),
                ((value_limb_9_col26.clone() + (M31_512.clone() * value_limb_10_col27.clone()))
                    + (M31_262144.clone() * value_limb_11_col28.clone())),
                ((value_limb_12_col29.clone() + (M31_512.clone() * value_limb_13_col30.clone()))
                    + (M31_262144.clone() * value_limb_14_col31.clone())),
                ((value_limb_15_col32.clone() + (M31_512.clone() * value_limb_16_col33.clone()))
                    + (M31_262144.clone() * value_limb_17_col34.clone())),
                ((value_limb_18_col35.clone() + (M31_512.clone() * value_limb_19_col36.clone()))
                    + (M31_262144.clone() * value_limb_20_col37.clone())),
                (value_limb_21_col38.clone() + (M31_512.clone() * value_limb_22_col39.clone())),
                ((value_limb_0_col41.clone() + (M31_512.clone() * value_limb_1_col42.clone()))
                    + (M31_262144.clone() * value_limb_2_col43.clone())),
                ((value_limb_3_col44.clone() + (M31_512.clone() * value_limb_4_col45.clone()))
                    + (M31_262144.clone() * value_limb_5_col46.clone())),
                ((value_limb_6_col47.clone() + (M31_512.clone() * value_limb_7_col48.clone()))
                    + (M31_262144.clone() * value_limb_8_col49.clone())),
                ((value_limb_9_col50.clone() + (M31_512.clone() * value_limb_10_col51.clone()))
                    + (M31_262144.clone() * value_limb_11_col52.clone())),
                ((value_limb_12_col53.clone() + (M31_512.clone() * value_limb_13_col54.clone()))
                    + (M31_262144.clone() * value_limb_14_col55.clone())),
                ((value_limb_15_col56.clone() + (M31_512.clone() * value_limb_16_col57.clone()))
                    + (M31_262144.clone() * value_limb_17_col58.clone())),
                ((value_limb_18_col59.clone() + (M31_512.clone() * value_limb_19_col60.clone()))
                    + (M31_262144.clone() * value_limb_20_col61.clone())),
                (value_limb_21_col62.clone() + (M31_512.clone() * value_limb_22_col63.clone())),
                ((value_limb_0_col65.clone() + (M31_512.clone() * value_limb_1_col66.clone()))
                    + (M31_262144.clone() * value_limb_2_col67.clone())),
                ((value_limb_3_col68.clone() + (M31_512.clone() * value_limb_4_col69.clone()))
                    + (M31_262144.clone() * value_limb_5_col70.clone())),
                ((value_limb_6_col71.clone() + (M31_512.clone() * value_limb_7_col72.clone()))
                    + (M31_262144.clone() * value_limb_8_col73.clone())),
                ((value_limb_9_col74.clone() + (M31_512.clone() * value_limb_10_col75.clone()))
                    + (M31_262144.clone() * value_limb_11_col76.clone())),
                ((value_limb_12_col77.clone() + (M31_512.clone() * value_limb_13_col78.clone()))
                    + (M31_262144.clone() * value_limb_14_col79.clone())),
                ((value_limb_15_col80.clone() + (M31_512.clone() * value_limb_16_col81.clone()))
                    + (M31_262144.clone() * value_limb_17_col82.clone())),
                ((value_limb_18_col83.clone() + (M31_512.clone() * value_limb_19_col84.clone()))
                    + (M31_262144.clone() * value_limb_20_col85.clone())),
                (value_limb_21_col86.clone() + (M31_512.clone() * value_limb_22_col87.clone())),
                ((value_limb_0_col89.clone() + (M31_512.clone() * value_limb_1_col90.clone()))
                    + (M31_262144.clone() * value_limb_2_col91.clone())),
                ((value_limb_3_col92.clone() + (M31_512.clone() * value_limb_4_col93.clone()))
                    + (M31_262144.clone() * value_limb_5_col94.clone())),
                ((value_limb_6_col95.clone() + (M31_512.clone() * value_limb_7_col96.clone()))
                    + (M31_262144.clone() * value_limb_8_col97.clone())),
                ((value_limb_9_col98.clone() + (M31_512.clone() * value_limb_10_col99.clone()))
                    + (M31_262144.clone() * value_limb_11_col100.clone())),
                ((value_limb_12_col101.clone() + (M31_512.clone() * value_limb_13_col102.clone()))
                    + (M31_262144.clone() * value_limb_14_col103.clone())),
                ((value_limb_15_col104.clone() + (M31_512.clone() * value_limb_16_col105.clone()))
                    + (M31_262144.clone() * value_limb_17_col106.clone())),
                ((value_limb_18_col107.clone() + (M31_512.clone() * value_limb_19_col108.clone()))
                    + (M31_262144.clone() * value_limb_20_col109.clone())),
                (value_limb_21_col110.clone() + (M31_512.clone() * value_limb_22_col111.clone())),
                ((value_limb_0_col113.clone() + (M31_512.clone() * value_limb_1_col114.clone()))
                    + (M31_262144.clone() * value_limb_2_col115.clone())),
                ((value_limb_3_col116.clone() + (M31_512.clone() * value_limb_4_col117.clone()))
                    + (M31_262144.clone() * value_limb_5_col118.clone())),
                ((value_limb_6_col119.clone() + (M31_512.clone() * value_limb_7_col120.clone()))
                    + (M31_262144.clone() * value_limb_8_col121.clone())),
                ((value_limb_9_col122.clone() + (M31_512.clone() * value_limb_10_col123.clone()))
                    + (M31_262144.clone() * value_limb_11_col124.clone())),
                ((value_limb_12_col125.clone() + (M31_512.clone() * value_limb_13_col126.clone()))
                    + (M31_262144.clone() * value_limb_14_col127.clone())),
                ((value_limb_15_col128.clone() + (M31_512.clone() * value_limb_16_col129.clone()))
                    + (M31_262144.clone() * value_limb_17_col130.clone())),
                ((value_limb_18_col131.clone() + (M31_512.clone() * value_limb_19_col132.clone()))
                    + (M31_262144.clone() * value_limb_20_col133.clone())),
                (value_limb_21_col134.clone() + (M31_512.clone() * value_limb_22_col135.clone())),
                ((value_limb_0_col137.clone() + (M31_512.clone() * value_limb_1_col138.clone()))
                    + (M31_262144.clone() * value_limb_2_col139.clone())),
                ((value_limb_3_col140.clone() + (M31_512.clone() * value_limb_4_col141.clone()))
                    + (M31_262144.clone() * value_limb_5_col142.clone())),
                ((value_limb_6_col143.clone() + (M31_512.clone() * value_limb_7_col144.clone()))
                    + (M31_262144.clone() * value_limb_8_col145.clone())),
                ((value_limb_9_col146.clone() + (M31_512.clone() * value_limb_10_col147.clone()))
                    + (M31_262144.clone() * value_limb_11_col148.clone())),
                ((value_limb_12_col149.clone() + (M31_512.clone() * value_limb_13_col150.clone()))
                    + (M31_262144.clone() * value_limb_14_col151.clone())),
                ((value_limb_15_col152.clone() + (M31_512.clone() * value_limb_16_col153.clone()))
                    + (M31_262144.clone() * value_limb_17_col154.clone())),
                ((value_limb_18_col155.clone() + (M31_512.clone() * value_limb_19_col156.clone()))
                    + (M31_262144.clone() * value_limb_20_col157.clone())),
                (value_limb_21_col158.clone() + (M31_512.clone() * value_limb_22_col159.clone())),
                ((value_limb_0_col161.clone() + (M31_512.clone() * value_limb_1_col162.clone()))
                    + (M31_262144.clone() * value_limb_2_col163.clone())),
                ((value_limb_3_col164.clone() + (M31_512.clone() * value_limb_4_col165.clone()))
                    + (M31_262144.clone() * value_limb_5_col166.clone())),
                ((value_limb_6_col167.clone() + (M31_512.clone() * value_limb_7_col168.clone()))
                    + (M31_262144.clone() * value_limb_8_col169.clone())),
                ((value_limb_9_col170.clone() + (M31_512.clone() * value_limb_10_col171.clone()))
                    + (M31_262144.clone() * value_limb_11_col172.clone())),
                ((value_limb_12_col173.clone() + (M31_512.clone() * value_limb_13_col174.clone()))
                    + (M31_262144.clone() * value_limb_14_col175.clone())),
                ((value_limb_15_col176.clone() + (M31_512.clone() * value_limb_16_col177.clone()))
                    + (M31_262144.clone() * value_limb_17_col178.clone())),
                ((value_limb_18_col179.clone() + (M31_512.clone() * value_limb_19_col180.clone()))
                    + (M31_262144.clone() * value_limb_20_col181.clone())),
                (value_limb_21_col182.clone() + (M31_512.clone() * value_limb_22_col183.clone())),
                ((value_limb_0_col185.clone() + (M31_512.clone() * value_limb_1_col186.clone()))
                    + (M31_262144.clone() * value_limb_2_col187.clone())),
                ((value_limb_3_col188.clone() + (M31_512.clone() * value_limb_4_col189.clone()))
                    + (M31_262144.clone() * value_limb_5_col190.clone())),
                ((value_limb_6_col191.clone() + (M31_512.clone() * value_limb_7_col192.clone()))
                    + (M31_262144.clone() * value_limb_8_col193.clone())),
                ((value_limb_9_col194.clone() + (M31_512.clone() * value_limb_10_col195.clone()))
                    + (M31_262144.clone() * value_limb_11_col196.clone())),
                ((value_limb_12_col197.clone() + (M31_512.clone() * value_limb_13_col198.clone()))
                    + (M31_262144.clone() * value_limb_14_col199.clone())),
                ((value_limb_15_col200.clone() + (M31_512.clone() * value_limb_16_col201.clone()))
                    + (M31_262144.clone() * value_limb_17_col202.clone())),
                ((value_limb_18_col203.clone() + (M31_512.clone() * value_limb_19_col204.clone()))
                    + (M31_262144.clone() * value_limb_20_col205.clone())),
                (value_limb_21_col206.clone() + (M31_512.clone() * value_limb_22_col207.clone())),
            ],
        ));

        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_384052509.clone(),
                seq.clone(),
                M31_24.clone(),
                keccak_round_output_limb_0_col209.clone(),
                keccak_round_output_limb_1_col210.clone(),
                keccak_round_output_limb_2_col211.clone(),
                keccak_round_output_limb_3_col212.clone(),
                keccak_round_output_limb_4_col213.clone(),
                keccak_round_output_limb_5_col214.clone(),
                keccak_round_output_limb_6_col215.clone(),
                keccak_round_output_limb_7_col216.clone(),
                keccak_round_output_limb_8_col217.clone(),
                keccak_round_output_limb_9_col218.clone(),
                keccak_round_output_limb_10_col219.clone(),
                keccak_round_output_limb_11_col220.clone(),
                keccak_round_output_limb_12_col221.clone(),
                keccak_round_output_limb_13_col222.clone(),
                keccak_round_output_limb_14_col223.clone(),
                keccak_round_output_limb_15_col224.clone(),
                keccak_round_output_limb_16_col225.clone(),
                keccak_round_output_limb_17_col226.clone(),
                keccak_round_output_limb_18_col227.clone(),
                keccak_round_output_limb_19_col228.clone(),
                keccak_round_output_limb_20_col229.clone(),
                keccak_round_output_limb_21_col230.clone(),
                keccak_round_output_limb_22_col231.clone(),
                keccak_round_output_limb_23_col232.clone(),
                keccak_round_output_limb_24_col233.clone(),
                keccak_round_output_limb_25_col234.clone(),
                keccak_round_output_limb_26_col235.clone(),
                keccak_round_output_limb_27_col236.clone(),
                keccak_round_output_limb_28_col237.clone(),
                keccak_round_output_limb_29_col238.clone(),
                keccak_round_output_limb_30_col239.clone(),
                keccak_round_output_limb_31_col240.clone(),
                keccak_round_output_limb_32_col241.clone(),
                keccak_round_output_limb_33_col242.clone(),
                keccak_round_output_limb_34_col243.clone(),
                keccak_round_output_limb_35_col244.clone(),
                keccak_round_output_limb_36_col245.clone(),
                keccak_round_output_limb_37_col246.clone(),
                keccak_round_output_limb_38_col247.clone(),
                keccak_round_output_limb_39_col248.clone(),
                keccak_round_output_limb_40_col249.clone(),
                keccak_round_output_limb_41_col250.clone(),
                keccak_round_output_limb_42_col251.clone(),
                keccak_round_output_limb_43_col252.clone(),
                keccak_round_output_limb_44_col253.clone(),
                keccak_round_output_limb_45_col254.clone(),
                keccak_round_output_limb_46_col255.clone(),
                keccak_round_output_limb_47_col256.clone(),
                keccak_round_output_limb_48_col257.clone(),
                keccak_round_output_limb_49_col258.clone(),
                keccak_round_output_limb_50_col259.clone(),
                keccak_round_output_limb_51_col260.clone(),
                keccak_round_output_limb_52_col261.clone(),
                keccak_round_output_limb_53_col262.clone(),
                keccak_round_output_limb_54_col263.clone(),
                keccak_round_output_limb_55_col264.clone(),
                keccak_round_output_limb_56_col265.clone(),
                keccak_round_output_limb_57_col266.clone(),
                keccak_round_output_limb_58_col267.clone(),
                keccak_round_output_limb_59_col268.clone(),
                keccak_round_output_limb_60_col269.clone(),
                keccak_round_output_limb_61_col270.clone(),
                keccak_round_output_limb_62_col271.clone(),
                keccak_round_output_limb_63_col272.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_59 = eval.add_intermediate(
            (((keccak_round_output_limb_0_col209.clone() - output_memory_limb_col273.clone())
                - (M31_512.clone() * output_memory_limb_col274.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_62 = eval.add_intermediate(
            (((keccak_round_output_limb_1_col210.clone() - output_memory_limb_col275.clone())
                - (M31_512.clone() * output_memory_limb_col276.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_65 = eval.add_intermediate(
            (((keccak_round_output_limb_2_col211.clone() - output_memory_limb_col277.clone())
                - (M31_512.clone() * output_memory_limb_col278.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_68 = eval.add_intermediate(
            (((keccak_round_output_limb_3_col212.clone() - output_memory_limb_col279.clone())
                - (M31_512.clone() * output_memory_limb_col280.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_71 = eval.add_intermediate(
            (((keccak_round_output_limb_4_col213.clone() - output_memory_limb_col281.clone())
                - (M31_512.clone() * output_memory_limb_col282.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_74 = eval.add_intermediate(
            (((keccak_round_output_limb_5_col214.clone() - output_memory_limb_col283.clone())
                - (M31_512.clone() * output_memory_limb_col284.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_77 = eval.add_intermediate(
            (((keccak_round_output_limb_6_col215.clone() - output_memory_limb_col285.clone())
                - (M31_512.clone() * output_memory_limb_col286.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_79 = eval.add_intermediate(
            ((keccak_round_output_limb_7_col216.clone() - output_memory_limb_col287.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_8_col9.clone(),
                output_memory_limb_col273.clone(),
                output_memory_limb_col274.clone(),
                output_memory_limb_last_tmp_8d1be_59.clone(),
                output_memory_limb_col275.clone(),
                output_memory_limb_col276.clone(),
                output_memory_limb_last_tmp_8d1be_62.clone(),
                output_memory_limb_col277.clone(),
                output_memory_limb_col278.clone(),
                output_memory_limb_last_tmp_8d1be_65.clone(),
                output_memory_limb_col279.clone(),
                output_memory_limb_col280.clone(),
                output_memory_limb_last_tmp_8d1be_68.clone(),
                output_memory_limb_col281.clone(),
                output_memory_limb_col282.clone(),
                output_memory_limb_last_tmp_8d1be_71.clone(),
                output_memory_limb_col283.clone(),
                output_memory_limb_col284.clone(),
                output_memory_limb_last_tmp_8d1be_74.clone(),
                output_memory_limb_col285.clone(),
                output_memory_limb_col286.clone(),
                output_memory_limb_last_tmp_8d1be_77.clone(),
                output_memory_limb_col287.clone(),
                output_memory_limb_last_tmp_8d1be_79.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_82 = eval.add_intermediate(
            (((keccak_round_output_limb_8_col217.clone() - output_memory_limb_col288.clone())
                - (M31_512.clone() * output_memory_limb_col289.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_85 = eval.add_intermediate(
            (((keccak_round_output_limb_9_col218.clone() - output_memory_limb_col290.clone())
                - (M31_512.clone() * output_memory_limb_col291.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_88 = eval.add_intermediate(
            (((keccak_round_output_limb_10_col219.clone() - output_memory_limb_col292.clone())
                - (M31_512.clone() * output_memory_limb_col293.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_91 = eval.add_intermediate(
            (((keccak_round_output_limb_11_col220.clone() - output_memory_limb_col294.clone())
                - (M31_512.clone() * output_memory_limb_col295.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_94 = eval.add_intermediate(
            (((keccak_round_output_limb_12_col221.clone() - output_memory_limb_col296.clone())
                - (M31_512.clone() * output_memory_limb_col297.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_97 = eval.add_intermediate(
            (((keccak_round_output_limb_13_col222.clone() - output_memory_limb_col298.clone())
                - (M31_512.clone() * output_memory_limb_col299.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_100 = eval.add_intermediate(
            (((keccak_round_output_limb_14_col223.clone() - output_memory_limb_col300.clone())
                - (M31_512.clone() * output_memory_limb_col301.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_102 = eval.add_intermediate(
            ((keccak_round_output_limb_15_col224.clone() - output_memory_limb_col302.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_9_col10.clone(),
                output_memory_limb_col288.clone(),
                output_memory_limb_col289.clone(),
                output_memory_limb_last_tmp_8d1be_82.clone(),
                output_memory_limb_col290.clone(),
                output_memory_limb_col291.clone(),
                output_memory_limb_last_tmp_8d1be_85.clone(),
                output_memory_limb_col292.clone(),
                output_memory_limb_col293.clone(),
                output_memory_limb_last_tmp_8d1be_88.clone(),
                output_memory_limb_col294.clone(),
                output_memory_limb_col295.clone(),
                output_memory_limb_last_tmp_8d1be_91.clone(),
                output_memory_limb_col296.clone(),
                output_memory_limb_col297.clone(),
                output_memory_limb_last_tmp_8d1be_94.clone(),
                output_memory_limb_col298.clone(),
                output_memory_limb_col299.clone(),
                output_memory_limb_last_tmp_8d1be_97.clone(),
                output_memory_limb_col300.clone(),
                output_memory_limb_col301.clone(),
                output_memory_limb_last_tmp_8d1be_100.clone(),
                output_memory_limb_col302.clone(),
                output_memory_limb_last_tmp_8d1be_102.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_105 = eval.add_intermediate(
            (((keccak_round_output_limb_16_col225.clone() - output_memory_limb_col303.clone())
                - (M31_512.clone() * output_memory_limb_col304.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_108 = eval.add_intermediate(
            (((keccak_round_output_limb_17_col226.clone() - output_memory_limb_col305.clone())
                - (M31_512.clone() * output_memory_limb_col306.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_111 = eval.add_intermediate(
            (((keccak_round_output_limb_18_col227.clone() - output_memory_limb_col307.clone())
                - (M31_512.clone() * output_memory_limb_col308.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_114 = eval.add_intermediate(
            (((keccak_round_output_limb_19_col228.clone() - output_memory_limb_col309.clone())
                - (M31_512.clone() * output_memory_limb_col310.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_117 = eval.add_intermediate(
            (((keccak_round_output_limb_20_col229.clone() - output_memory_limb_col311.clone())
                - (M31_512.clone() * output_memory_limb_col312.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_120 = eval.add_intermediate(
            (((keccak_round_output_limb_21_col230.clone() - output_memory_limb_col313.clone())
                - (M31_512.clone() * output_memory_limb_col314.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_123 = eval.add_intermediate(
            (((keccak_round_output_limb_22_col231.clone() - output_memory_limb_col315.clone())
                - (M31_512.clone() * output_memory_limb_col316.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_125 = eval.add_intermediate(
            ((keccak_round_output_limb_23_col232.clone() - output_memory_limb_col317.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_10_col11.clone(),
                output_memory_limb_col303.clone(),
                output_memory_limb_col304.clone(),
                output_memory_limb_last_tmp_8d1be_105.clone(),
                output_memory_limb_col305.clone(),
                output_memory_limb_col306.clone(),
                output_memory_limb_last_tmp_8d1be_108.clone(),
                output_memory_limb_col307.clone(),
                output_memory_limb_col308.clone(),
                output_memory_limb_last_tmp_8d1be_111.clone(),
                output_memory_limb_col309.clone(),
                output_memory_limb_col310.clone(),
                output_memory_limb_last_tmp_8d1be_114.clone(),
                output_memory_limb_col311.clone(),
                output_memory_limb_col312.clone(),
                output_memory_limb_last_tmp_8d1be_117.clone(),
                output_memory_limb_col313.clone(),
                output_memory_limb_col314.clone(),
                output_memory_limb_last_tmp_8d1be_120.clone(),
                output_memory_limb_col315.clone(),
                output_memory_limb_col316.clone(),
                output_memory_limb_last_tmp_8d1be_123.clone(),
                output_memory_limb_col317.clone(),
                output_memory_limb_last_tmp_8d1be_125.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_128 = eval.add_intermediate(
            (((keccak_round_output_limb_24_col233.clone() - output_memory_limb_col318.clone())
                - (M31_512.clone() * output_memory_limb_col319.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_131 = eval.add_intermediate(
            (((keccak_round_output_limb_25_col234.clone() - output_memory_limb_col320.clone())
                - (M31_512.clone() * output_memory_limb_col321.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_134 = eval.add_intermediate(
            (((keccak_round_output_limb_26_col235.clone() - output_memory_limb_col322.clone())
                - (M31_512.clone() * output_memory_limb_col323.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_137 = eval.add_intermediate(
            (((keccak_round_output_limb_27_col236.clone() - output_memory_limb_col324.clone())
                - (M31_512.clone() * output_memory_limb_col325.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_140 = eval.add_intermediate(
            (((keccak_round_output_limb_28_col237.clone() - output_memory_limb_col326.clone())
                - (M31_512.clone() * output_memory_limb_col327.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_143 = eval.add_intermediate(
            (((keccak_round_output_limb_29_col238.clone() - output_memory_limb_col328.clone())
                - (M31_512.clone() * output_memory_limb_col329.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_146 = eval.add_intermediate(
            (((keccak_round_output_limb_30_col239.clone() - output_memory_limb_col330.clone())
                - (M31_512.clone() * output_memory_limb_col331.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_148 = eval.add_intermediate(
            ((keccak_round_output_limb_31_col240.clone() - output_memory_limb_col332.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_11_col12.clone(),
                output_memory_limb_col318.clone(),
                output_memory_limb_col319.clone(),
                output_memory_limb_last_tmp_8d1be_128.clone(),
                output_memory_limb_col320.clone(),
                output_memory_limb_col321.clone(),
                output_memory_limb_last_tmp_8d1be_131.clone(),
                output_memory_limb_col322.clone(),
                output_memory_limb_col323.clone(),
                output_memory_limb_last_tmp_8d1be_134.clone(),
                output_memory_limb_col324.clone(),
                output_memory_limb_col325.clone(),
                output_memory_limb_last_tmp_8d1be_137.clone(),
                output_memory_limb_col326.clone(),
                output_memory_limb_col327.clone(),
                output_memory_limb_last_tmp_8d1be_140.clone(),
                output_memory_limb_col328.clone(),
                output_memory_limb_col329.clone(),
                output_memory_limb_last_tmp_8d1be_143.clone(),
                output_memory_limb_col330.clone(),
                output_memory_limb_col331.clone(),
                output_memory_limb_last_tmp_8d1be_146.clone(),
                output_memory_limb_col332.clone(),
                output_memory_limb_last_tmp_8d1be_148.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_151 = eval.add_intermediate(
            (((keccak_round_output_limb_32_col241.clone() - output_memory_limb_col333.clone())
                - (M31_512.clone() * output_memory_limb_col334.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_154 = eval.add_intermediate(
            (((keccak_round_output_limb_33_col242.clone() - output_memory_limb_col335.clone())
                - (M31_512.clone() * output_memory_limb_col336.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_157 = eval.add_intermediate(
            (((keccak_round_output_limb_34_col243.clone() - output_memory_limb_col337.clone())
                - (M31_512.clone() * output_memory_limb_col338.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_160 = eval.add_intermediate(
            (((keccak_round_output_limb_35_col244.clone() - output_memory_limb_col339.clone())
                - (M31_512.clone() * output_memory_limb_col340.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_163 = eval.add_intermediate(
            (((keccak_round_output_limb_36_col245.clone() - output_memory_limb_col341.clone())
                - (M31_512.clone() * output_memory_limb_col342.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_166 = eval.add_intermediate(
            (((keccak_round_output_limb_37_col246.clone() - output_memory_limb_col343.clone())
                - (M31_512.clone() * output_memory_limb_col344.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_169 = eval.add_intermediate(
            (((keccak_round_output_limb_38_col247.clone() - output_memory_limb_col345.clone())
                - (M31_512.clone() * output_memory_limb_col346.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_171 = eval.add_intermediate(
            ((keccak_round_output_limb_39_col248.clone() - output_memory_limb_col347.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_12_col13.clone(),
                output_memory_limb_col333.clone(),
                output_memory_limb_col334.clone(),
                output_memory_limb_last_tmp_8d1be_151.clone(),
                output_memory_limb_col335.clone(),
                output_memory_limb_col336.clone(),
                output_memory_limb_last_tmp_8d1be_154.clone(),
                output_memory_limb_col337.clone(),
                output_memory_limb_col338.clone(),
                output_memory_limb_last_tmp_8d1be_157.clone(),
                output_memory_limb_col339.clone(),
                output_memory_limb_col340.clone(),
                output_memory_limb_last_tmp_8d1be_160.clone(),
                output_memory_limb_col341.clone(),
                output_memory_limb_col342.clone(),
                output_memory_limb_last_tmp_8d1be_163.clone(),
                output_memory_limb_col343.clone(),
                output_memory_limb_col344.clone(),
                output_memory_limb_last_tmp_8d1be_166.clone(),
                output_memory_limb_col345.clone(),
                output_memory_limb_col346.clone(),
                output_memory_limb_last_tmp_8d1be_169.clone(),
                output_memory_limb_col347.clone(),
                output_memory_limb_last_tmp_8d1be_171.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_174 = eval.add_intermediate(
            (((keccak_round_output_limb_40_col249.clone() - output_memory_limb_col348.clone())
                - (M31_512.clone() * output_memory_limb_col349.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_177 = eval.add_intermediate(
            (((keccak_round_output_limb_41_col250.clone() - output_memory_limb_col350.clone())
                - (M31_512.clone() * output_memory_limb_col351.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_180 = eval.add_intermediate(
            (((keccak_round_output_limb_42_col251.clone() - output_memory_limb_col352.clone())
                - (M31_512.clone() * output_memory_limb_col353.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_183 = eval.add_intermediate(
            (((keccak_round_output_limb_43_col252.clone() - output_memory_limb_col354.clone())
                - (M31_512.clone() * output_memory_limb_col355.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_186 = eval.add_intermediate(
            (((keccak_round_output_limb_44_col253.clone() - output_memory_limb_col356.clone())
                - (M31_512.clone() * output_memory_limb_col357.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_189 = eval.add_intermediate(
            (((keccak_round_output_limb_45_col254.clone() - output_memory_limb_col358.clone())
                - (M31_512.clone() * output_memory_limb_col359.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_192 = eval.add_intermediate(
            (((keccak_round_output_limb_46_col255.clone() - output_memory_limb_col360.clone())
                - (M31_512.clone() * output_memory_limb_col361.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_194 = eval.add_intermediate(
            ((keccak_round_output_limb_47_col256.clone() - output_memory_limb_col362.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_13_col14.clone(),
                output_memory_limb_col348.clone(),
                output_memory_limb_col349.clone(),
                output_memory_limb_last_tmp_8d1be_174.clone(),
                output_memory_limb_col350.clone(),
                output_memory_limb_col351.clone(),
                output_memory_limb_last_tmp_8d1be_177.clone(),
                output_memory_limb_col352.clone(),
                output_memory_limb_col353.clone(),
                output_memory_limb_last_tmp_8d1be_180.clone(),
                output_memory_limb_col354.clone(),
                output_memory_limb_col355.clone(),
                output_memory_limb_last_tmp_8d1be_183.clone(),
                output_memory_limb_col356.clone(),
                output_memory_limb_col357.clone(),
                output_memory_limb_last_tmp_8d1be_186.clone(),
                output_memory_limb_col358.clone(),
                output_memory_limb_col359.clone(),
                output_memory_limb_last_tmp_8d1be_189.clone(),
                output_memory_limb_col360.clone(),
                output_memory_limb_col361.clone(),
                output_memory_limb_last_tmp_8d1be_192.clone(),
                output_memory_limb_col362.clone(),
                output_memory_limb_last_tmp_8d1be_194.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_197 = eval.add_intermediate(
            (((keccak_round_output_limb_48_col257.clone() - output_memory_limb_col363.clone())
                - (M31_512.clone() * output_memory_limb_col364.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_200 = eval.add_intermediate(
            (((keccak_round_output_limb_49_col258.clone() - output_memory_limb_col365.clone())
                - (M31_512.clone() * output_memory_limb_col366.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_203 = eval.add_intermediate(
            (((keccak_round_output_limb_50_col259.clone() - output_memory_limb_col367.clone())
                - (M31_512.clone() * output_memory_limb_col368.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_206 = eval.add_intermediate(
            (((keccak_round_output_limb_51_col260.clone() - output_memory_limb_col369.clone())
                - (M31_512.clone() * output_memory_limb_col370.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_209 = eval.add_intermediate(
            (((keccak_round_output_limb_52_col261.clone() - output_memory_limb_col371.clone())
                - (M31_512.clone() * output_memory_limb_col372.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_212 = eval.add_intermediate(
            (((keccak_round_output_limb_53_col262.clone() - output_memory_limb_col373.clone())
                - (M31_512.clone() * output_memory_limb_col374.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_215 = eval.add_intermediate(
            (((keccak_round_output_limb_54_col263.clone() - output_memory_limb_col375.clone())
                - (M31_512.clone() * output_memory_limb_col376.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_217 = eval.add_intermediate(
            ((keccak_round_output_limb_55_col264.clone() - output_memory_limb_col377.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_14_col15.clone(),
                output_memory_limb_col363.clone(),
                output_memory_limb_col364.clone(),
                output_memory_limb_last_tmp_8d1be_197.clone(),
                output_memory_limb_col365.clone(),
                output_memory_limb_col366.clone(),
                output_memory_limb_last_tmp_8d1be_200.clone(),
                output_memory_limb_col367.clone(),
                output_memory_limb_col368.clone(),
                output_memory_limb_last_tmp_8d1be_203.clone(),
                output_memory_limb_col369.clone(),
                output_memory_limb_col370.clone(),
                output_memory_limb_last_tmp_8d1be_206.clone(),
                output_memory_limb_col371.clone(),
                output_memory_limb_col372.clone(),
                output_memory_limb_last_tmp_8d1be_209.clone(),
                output_memory_limb_col373.clone(),
                output_memory_limb_col374.clone(),
                output_memory_limb_last_tmp_8d1be_212.clone(),
                output_memory_limb_col375.clone(),
                output_memory_limb_col376.clone(),
                output_memory_limb_last_tmp_8d1be_215.clone(),
                output_memory_limb_col377.clone(),
                output_memory_limb_last_tmp_8d1be_217.clone(),
            ],
        ));

        let output_memory_limb_last_tmp_8d1be_220 = eval.add_intermediate(
            (((keccak_round_output_limb_56_col265.clone() - output_memory_limb_col378.clone())
                - (M31_512.clone() * output_memory_limb_col379.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_223 = eval.add_intermediate(
            (((keccak_round_output_limb_57_col266.clone() - output_memory_limb_col380.clone())
                - (M31_512.clone() * output_memory_limb_col381.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_226 = eval.add_intermediate(
            (((keccak_round_output_limb_58_col267.clone() - output_memory_limb_col382.clone())
                - (M31_512.clone() * output_memory_limb_col383.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_229 = eval.add_intermediate(
            (((keccak_round_output_limb_59_col268.clone() - output_memory_limb_col384.clone())
                - (M31_512.clone() * output_memory_limb_col385.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_232 = eval.add_intermediate(
            (((keccak_round_output_limb_60_col269.clone() - output_memory_limb_col386.clone())
                - (M31_512.clone() * output_memory_limb_col387.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_235 = eval.add_intermediate(
            (((keccak_round_output_limb_61_col270.clone() - output_memory_limb_col388.clone())
                - (M31_512.clone() * output_memory_limb_col389.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_238 = eval.add_intermediate(
            (((keccak_round_output_limb_62_col271.clone() - output_memory_limb_col390.clone())
                - (M31_512.clone() * output_memory_limb_col391.clone()))
                * M31_8192.clone()),
        );
        let output_memory_limb_last_tmp_8d1be_240 = eval.add_intermediate(
            ((keccak_round_output_limb_63_col272.clone() - output_memory_limb_col392.clone())
                * M31_4194304.clone()),
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1662111297.clone(),
                input_limb_15_col16.clone(),
                output_memory_limb_col378.clone(),
                output_memory_limb_col379.clone(),
                output_memory_limb_last_tmp_8d1be_220.clone(),
                output_memory_limb_col380.clone(),
                output_memory_limb_col381.clone(),
                output_memory_limb_last_tmp_8d1be_223.clone(),
                output_memory_limb_col382.clone(),
                output_memory_limb_col383.clone(),
                output_memory_limb_last_tmp_8d1be_226.clone(),
                output_memory_limb_col384.clone(),
                output_memory_limb_col385.clone(),
                output_memory_limb_last_tmp_8d1be_229.clone(),
                output_memory_limb_col386.clone(),
                output_memory_limb_col387.clone(),
                output_memory_limb_last_tmp_8d1be_232.clone(),
                output_memory_limb_col388.clone(),
                output_memory_limb_col389.clone(),
                output_memory_limb_last_tmp_8d1be_235.clone(),
                output_memory_limb_col390.clone(),
                output_memory_limb_col391.clone(),
                output_memory_limb_last_tmp_8d1be_238.clone(),
                output_memory_limb_col392.clone(),
                output_memory_limb_last_tmp_8d1be_240.clone(),
            ],
        ));

        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            -E::EF::from(multiplicity_0_col0.clone()),
            &[
                M31_1973115366.clone(),
                input_limb_0_col1.clone(),
                input_limb_1_col2.clone(),
                input_limb_2_col3.clone(),
                input_limb_3_col4.clone(),
                input_limb_4_col5.clone(),
                input_limb_5_col6.clone(),
                input_limb_6_col7.clone(),
                input_limb_7_col8.clone(),
                input_limb_8_col9.clone(),
                input_limb_9_col10.clone(),
                input_limb_10_col11.clone(),
                input_limb_11_col12.clone(),
                input_limb_12_col13.clone(),
                input_limb_13_col14.clone(),
                input_limb_14_col15.clone(),
                input_limb_15_col16.clone(),
            ],
        ));

        eval.finalize_logup_in_pairs();
        eval
    }
}

#[cfg(test)]
mod tests {
    use num_traits::Zero;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
    use stwo::core::fields::qm31::QM31;
    use stwo_constraint_framework::expr::ExprEvaluator;

    use super::*;

    #[test]
    fn keccak_aggregator_constraints_regression() {
        let mut rng = SmallRng::seed_from_u64(0);
        let eval = Eval {
            claim: Claim { log_size: 4 },
            common_lookup_elements: relations::CommonLookupElements::dummy(),
        };
        let expr_eval = eval.evaluate(ExprEvaluator::new());
        let assignment = expr_eval.random_assignment();

        let mut sum = QM31::zero();
        for c in expr_eval.constraints {
            sum += c.assign(&assignment) * rng.random::<QM31>();
        }

        constraints_regression_test_values::KECCAK_AGGREGATOR.assert_debug_eq(&sum);
    }
}
