// This file was created by the AIR team.

use super::prelude::*;

pub const N_TRACE_COLUMNS: usize = 393;
pub const N_INTERACTION_COLUMNS: usize = 40;

pub const RELATION_USES_PER_ROW: [RelationUse; 2] = [
    RelationUse { relation_id: "KeccakRound", uses: 1 },
    RelationUse { relation_id: "MemoryIdToBig", uses: 16 },
];

#[allow(unused_variables)]
pub fn accumulate_constraints<Value: IValue>(
    input: &[Var],
    context: &mut Context<Value>,
    component_data: &dyn ComponentDataTrait<Value>,
    acc: &mut CompositionConstraintAccumulator,
) {
    let [
        multiplicity_0_col0,
        input_limb_0_col1,
        input_limb_1_col2,
        input_limb_2_col3,
        input_limb_3_col4,
        input_limb_4_col5,
        input_limb_5_col6,
        input_limb_6_col7,
        input_limb_7_col8,
        input_limb_8_col9,
        input_limb_9_col10,
        input_limb_10_col11,
        input_limb_11_col12,
        input_limb_12_col13,
        input_limb_13_col14,
        input_limb_14_col15,
        input_limb_15_col16,
        value_limb_0_col17,
        value_limb_1_col18,
        value_limb_2_col19,
        value_limb_3_col20,
        value_limb_4_col21,
        value_limb_5_col22,
        value_limb_6_col23,
        value_limb_7_col24,
        value_limb_8_col25,
        value_limb_9_col26,
        value_limb_10_col27,
        value_limb_11_col28,
        value_limb_12_col29,
        value_limb_13_col30,
        value_limb_14_col31,
        value_limb_15_col32,
        value_limb_16_col33,
        value_limb_17_col34,
        value_limb_18_col35,
        value_limb_19_col36,
        value_limb_20_col37,
        value_limb_21_col38,
        value_limb_22_col39,
        partial_limb_msb_col40,
        value_limb_0_col41,
        value_limb_1_col42,
        value_limb_2_col43,
        value_limb_3_col44,
        value_limb_4_col45,
        value_limb_5_col46,
        value_limb_6_col47,
        value_limb_7_col48,
        value_limb_8_col49,
        value_limb_9_col50,
        value_limb_10_col51,
        value_limb_11_col52,
        value_limb_12_col53,
        value_limb_13_col54,
        value_limb_14_col55,
        value_limb_15_col56,
        value_limb_16_col57,
        value_limb_17_col58,
        value_limb_18_col59,
        value_limb_19_col60,
        value_limb_20_col61,
        value_limb_21_col62,
        value_limb_22_col63,
        partial_limb_msb_col64,
        value_limb_0_col65,
        value_limb_1_col66,
        value_limb_2_col67,
        value_limb_3_col68,
        value_limb_4_col69,
        value_limb_5_col70,
        value_limb_6_col71,
        value_limb_7_col72,
        value_limb_8_col73,
        value_limb_9_col74,
        value_limb_10_col75,
        value_limb_11_col76,
        value_limb_12_col77,
        value_limb_13_col78,
        value_limb_14_col79,
        value_limb_15_col80,
        value_limb_16_col81,
        value_limb_17_col82,
        value_limb_18_col83,
        value_limb_19_col84,
        value_limb_20_col85,
        value_limb_21_col86,
        value_limb_22_col87,
        partial_limb_msb_col88,
        value_limb_0_col89,
        value_limb_1_col90,
        value_limb_2_col91,
        value_limb_3_col92,
        value_limb_4_col93,
        value_limb_5_col94,
        value_limb_6_col95,
        value_limb_7_col96,
        value_limb_8_col97,
        value_limb_9_col98,
        value_limb_10_col99,
        value_limb_11_col100,
        value_limb_12_col101,
        value_limb_13_col102,
        value_limb_14_col103,
        value_limb_15_col104,
        value_limb_16_col105,
        value_limb_17_col106,
        value_limb_18_col107,
        value_limb_19_col108,
        value_limb_20_col109,
        value_limb_21_col110,
        value_limb_22_col111,
        partial_limb_msb_col112,
        value_limb_0_col113,
        value_limb_1_col114,
        value_limb_2_col115,
        value_limb_3_col116,
        value_limb_4_col117,
        value_limb_5_col118,
        value_limb_6_col119,
        value_limb_7_col120,
        value_limb_8_col121,
        value_limb_9_col122,
        value_limb_10_col123,
        value_limb_11_col124,
        value_limb_12_col125,
        value_limb_13_col126,
        value_limb_14_col127,
        value_limb_15_col128,
        value_limb_16_col129,
        value_limb_17_col130,
        value_limb_18_col131,
        value_limb_19_col132,
        value_limb_20_col133,
        value_limb_21_col134,
        value_limb_22_col135,
        partial_limb_msb_col136,
        value_limb_0_col137,
        value_limb_1_col138,
        value_limb_2_col139,
        value_limb_3_col140,
        value_limb_4_col141,
        value_limb_5_col142,
        value_limb_6_col143,
        value_limb_7_col144,
        value_limb_8_col145,
        value_limb_9_col146,
        value_limb_10_col147,
        value_limb_11_col148,
        value_limb_12_col149,
        value_limb_13_col150,
        value_limb_14_col151,
        value_limb_15_col152,
        value_limb_16_col153,
        value_limb_17_col154,
        value_limb_18_col155,
        value_limb_19_col156,
        value_limb_20_col157,
        value_limb_21_col158,
        value_limb_22_col159,
        partial_limb_msb_col160,
        value_limb_0_col161,
        value_limb_1_col162,
        value_limb_2_col163,
        value_limb_3_col164,
        value_limb_4_col165,
        value_limb_5_col166,
        value_limb_6_col167,
        value_limb_7_col168,
        value_limb_8_col169,
        value_limb_9_col170,
        value_limb_10_col171,
        value_limb_11_col172,
        value_limb_12_col173,
        value_limb_13_col174,
        value_limb_14_col175,
        value_limb_15_col176,
        value_limb_16_col177,
        value_limb_17_col178,
        value_limb_18_col179,
        value_limb_19_col180,
        value_limb_20_col181,
        value_limb_21_col182,
        value_limb_22_col183,
        partial_limb_msb_col184,
        value_limb_0_col185,
        value_limb_1_col186,
        value_limb_2_col187,
        value_limb_3_col188,
        value_limb_4_col189,
        value_limb_5_col190,
        value_limb_6_col191,
        value_limb_7_col192,
        value_limb_8_col193,
        value_limb_9_col194,
        value_limb_10_col195,
        value_limb_11_col196,
        value_limb_12_col197,
        value_limb_13_col198,
        value_limb_14_col199,
        value_limb_15_col200,
        value_limb_16_col201,
        value_limb_17_col202,
        value_limb_18_col203,
        value_limb_19_col204,
        value_limb_20_col205,
        value_limb_21_col206,
        value_limb_22_col207,
        partial_limb_msb_col208,
        keccak_round_output_limb_0_col209,
        keccak_round_output_limb_1_col210,
        keccak_round_output_limb_2_col211,
        keccak_round_output_limb_3_col212,
        keccak_round_output_limb_4_col213,
        keccak_round_output_limb_5_col214,
        keccak_round_output_limb_6_col215,
        keccak_round_output_limb_7_col216,
        keccak_round_output_limb_8_col217,
        keccak_round_output_limb_9_col218,
        keccak_round_output_limb_10_col219,
        keccak_round_output_limb_11_col220,
        keccak_round_output_limb_12_col221,
        keccak_round_output_limb_13_col222,
        keccak_round_output_limb_14_col223,
        keccak_round_output_limb_15_col224,
        keccak_round_output_limb_16_col225,
        keccak_round_output_limb_17_col226,
        keccak_round_output_limb_18_col227,
        keccak_round_output_limb_19_col228,
        keccak_round_output_limb_20_col229,
        keccak_round_output_limb_21_col230,
        keccak_round_output_limb_22_col231,
        keccak_round_output_limb_23_col232,
        keccak_round_output_limb_24_col233,
        keccak_round_output_limb_25_col234,
        keccak_round_output_limb_26_col235,
        keccak_round_output_limb_27_col236,
        keccak_round_output_limb_28_col237,
        keccak_round_output_limb_29_col238,
        keccak_round_output_limb_30_col239,
        keccak_round_output_limb_31_col240,
        keccak_round_output_limb_32_col241,
        keccak_round_output_limb_33_col242,
        keccak_round_output_limb_34_col243,
        keccak_round_output_limb_35_col244,
        keccak_round_output_limb_36_col245,
        keccak_round_output_limb_37_col246,
        keccak_round_output_limb_38_col247,
        keccak_round_output_limb_39_col248,
        keccak_round_output_limb_40_col249,
        keccak_round_output_limb_41_col250,
        keccak_round_output_limb_42_col251,
        keccak_round_output_limb_43_col252,
        keccak_round_output_limb_44_col253,
        keccak_round_output_limb_45_col254,
        keccak_round_output_limb_46_col255,
        keccak_round_output_limb_47_col256,
        keccak_round_output_limb_48_col257,
        keccak_round_output_limb_49_col258,
        keccak_round_output_limb_50_col259,
        keccak_round_output_limb_51_col260,
        keccak_round_output_limb_52_col261,
        keccak_round_output_limb_53_col262,
        keccak_round_output_limb_54_col263,
        keccak_round_output_limb_55_col264,
        keccak_round_output_limb_56_col265,
        keccak_round_output_limb_57_col266,
        keccak_round_output_limb_58_col267,
        keccak_round_output_limb_59_col268,
        keccak_round_output_limb_60_col269,
        keccak_round_output_limb_61_col270,
        keccak_round_output_limb_62_col271,
        keccak_round_output_limb_63_col272,
        output_memory_limb_col273,
        output_memory_limb_col274,
        output_memory_limb_col275,
        output_memory_limb_col276,
        output_memory_limb_col277,
        output_memory_limb_col278,
        output_memory_limb_col279,
        output_memory_limb_col280,
        output_memory_limb_col281,
        output_memory_limb_col282,
        output_memory_limb_col283,
        output_memory_limb_col284,
        output_memory_limb_col285,
        output_memory_limb_col286,
        output_memory_limb_col287,
        output_memory_limb_col288,
        output_memory_limb_col289,
        output_memory_limb_col290,
        output_memory_limb_col291,
        output_memory_limb_col292,
        output_memory_limb_col293,
        output_memory_limb_col294,
        output_memory_limb_col295,
        output_memory_limb_col296,
        output_memory_limb_col297,
        output_memory_limb_col298,
        output_memory_limb_col299,
        output_memory_limb_col300,
        output_memory_limb_col301,
        output_memory_limb_col302,
        output_memory_limb_col303,
        output_memory_limb_col304,
        output_memory_limb_col305,
        output_memory_limb_col306,
        output_memory_limb_col307,
        output_memory_limb_col308,
        output_memory_limb_col309,
        output_memory_limb_col310,
        output_memory_limb_col311,
        output_memory_limb_col312,
        output_memory_limb_col313,
        output_memory_limb_col314,
        output_memory_limb_col315,
        output_memory_limb_col316,
        output_memory_limb_col317,
        output_memory_limb_col318,
        output_memory_limb_col319,
        output_memory_limb_col320,
        output_memory_limb_col321,
        output_memory_limb_col322,
        output_memory_limb_col323,
        output_memory_limb_col324,
        output_memory_limb_col325,
        output_memory_limb_col326,
        output_memory_limb_col327,
        output_memory_limb_col328,
        output_memory_limb_col329,
        output_memory_limb_col330,
        output_memory_limb_col331,
        output_memory_limb_col332,
        output_memory_limb_col333,
        output_memory_limb_col334,
        output_memory_limb_col335,
        output_memory_limb_col336,
        output_memory_limb_col337,
        output_memory_limb_col338,
        output_memory_limb_col339,
        output_memory_limb_col340,
        output_memory_limb_col341,
        output_memory_limb_col342,
        output_memory_limb_col343,
        output_memory_limb_col344,
        output_memory_limb_col345,
        output_memory_limb_col346,
        output_memory_limb_col347,
        output_memory_limb_col348,
        output_memory_limb_col349,
        output_memory_limb_col350,
        output_memory_limb_col351,
        output_memory_limb_col352,
        output_memory_limb_col353,
        output_memory_limb_col354,
        output_memory_limb_col355,
        output_memory_limb_col356,
        output_memory_limb_col357,
        output_memory_limb_col358,
        output_memory_limb_col359,
        output_memory_limb_col360,
        output_memory_limb_col361,
        output_memory_limb_col362,
        output_memory_limb_col363,
        output_memory_limb_col364,
        output_memory_limb_col365,
        output_memory_limb_col366,
        output_memory_limb_col367,
        output_memory_limb_col368,
        output_memory_limb_col369,
        output_memory_limb_col370,
        output_memory_limb_col371,
        output_memory_limb_col372,
        output_memory_limb_col373,
        output_memory_limb_col374,
        output_memory_limb_col375,
        output_memory_limb_col376,
        output_memory_limb_col377,
        output_memory_limb_col378,
        output_memory_limb_col379,
        output_memory_limb_col380,
        output_memory_limb_col381,
        output_memory_limb_col382,
        output_memory_limb_col383,
        output_memory_limb_col384,
        output_memory_limb_col385,
        output_memory_limb_col386,
        output_memory_limb_col387,
        output_memory_limb_col388,
        output_memory_limb_col389,
        output_memory_limb_col390,
        output_memory_limb_col391,
        output_memory_limb_col392,
    ] = input.try_into().unwrap();
    let seq = seq_of_component_size(context, component_data, &acc.preprocessed_columns);

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_0_col1),
            eval!(context, 1),
            eval!(context, value_limb_0_col17),
            eval!(context, value_limb_1_col18),
            eval!(context, value_limb_2_col19),
            eval!(context, value_limb_3_col20),
            eval!(context, value_limb_4_col21),
            eval!(context, value_limb_5_col22),
            eval!(context, value_limb_6_col23),
            eval!(context, value_limb_7_col24),
            eval!(context, value_limb_8_col25),
            eval!(context, value_limb_9_col26),
            eval!(context, value_limb_10_col27),
            eval!(context, value_limb_11_col28),
            eval!(context, value_limb_12_col29),
            eval!(context, value_limb_13_col30),
            eval!(context, value_limb_14_col31),
            eval!(context, value_limb_15_col32),
            eval!(context, value_limb_16_col33),
            eval!(context, value_limb_17_col34),
            eval!(context, value_limb_18_col35),
            eval!(context, value_limb_19_col36),
            eval!(context, value_limb_20_col37),
            eval!(context, value_limb_21_col38),
            eval!(context, value_limb_22_col39),
            eval!(context, partial_limb_msb_col40),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_1_col2),
            eval!(context, 1),
            eval!(context, value_limb_0_col41),
            eval!(context, value_limb_1_col42),
            eval!(context, value_limb_2_col43),
            eval!(context, value_limb_3_col44),
            eval!(context, value_limb_4_col45),
            eval!(context, value_limb_5_col46),
            eval!(context, value_limb_6_col47),
            eval!(context, value_limb_7_col48),
            eval!(context, value_limb_8_col49),
            eval!(context, value_limb_9_col50),
            eval!(context, value_limb_10_col51),
            eval!(context, value_limb_11_col52),
            eval!(context, value_limb_12_col53),
            eval!(context, value_limb_13_col54),
            eval!(context, value_limb_14_col55),
            eval!(context, value_limb_15_col56),
            eval!(context, value_limb_16_col57),
            eval!(context, value_limb_17_col58),
            eval!(context, value_limb_18_col59),
            eval!(context, value_limb_19_col60),
            eval!(context, value_limb_20_col61),
            eval!(context, value_limb_21_col62),
            eval!(context, value_limb_22_col63),
            eval!(context, partial_limb_msb_col64),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_2_col3),
            eval!(context, 1),
            eval!(context, value_limb_0_col65),
            eval!(context, value_limb_1_col66),
            eval!(context, value_limb_2_col67),
            eval!(context, value_limb_3_col68),
            eval!(context, value_limb_4_col69),
            eval!(context, value_limb_5_col70),
            eval!(context, value_limb_6_col71),
            eval!(context, value_limb_7_col72),
            eval!(context, value_limb_8_col73),
            eval!(context, value_limb_9_col74),
            eval!(context, value_limb_10_col75),
            eval!(context, value_limb_11_col76),
            eval!(context, value_limb_12_col77),
            eval!(context, value_limb_13_col78),
            eval!(context, value_limb_14_col79),
            eval!(context, value_limb_15_col80),
            eval!(context, value_limb_16_col81),
            eval!(context, value_limb_17_col82),
            eval!(context, value_limb_18_col83),
            eval!(context, value_limb_19_col84),
            eval!(context, value_limb_20_col85),
            eval!(context, value_limb_21_col86),
            eval!(context, value_limb_22_col87),
            eval!(context, partial_limb_msb_col88),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_3_col4),
            eval!(context, 1),
            eval!(context, value_limb_0_col89),
            eval!(context, value_limb_1_col90),
            eval!(context, value_limb_2_col91),
            eval!(context, value_limb_3_col92),
            eval!(context, value_limb_4_col93),
            eval!(context, value_limb_5_col94),
            eval!(context, value_limb_6_col95),
            eval!(context, value_limb_7_col96),
            eval!(context, value_limb_8_col97),
            eval!(context, value_limb_9_col98),
            eval!(context, value_limb_10_col99),
            eval!(context, value_limb_11_col100),
            eval!(context, value_limb_12_col101),
            eval!(context, value_limb_13_col102),
            eval!(context, value_limb_14_col103),
            eval!(context, value_limb_15_col104),
            eval!(context, value_limb_16_col105),
            eval!(context, value_limb_17_col106),
            eval!(context, value_limb_18_col107),
            eval!(context, value_limb_19_col108),
            eval!(context, value_limb_20_col109),
            eval!(context, value_limb_21_col110),
            eval!(context, value_limb_22_col111),
            eval!(context, partial_limb_msb_col112),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_4_col5),
            eval!(context, 1),
            eval!(context, value_limb_0_col113),
            eval!(context, value_limb_1_col114),
            eval!(context, value_limb_2_col115),
            eval!(context, value_limb_3_col116),
            eval!(context, value_limb_4_col117),
            eval!(context, value_limb_5_col118),
            eval!(context, value_limb_6_col119),
            eval!(context, value_limb_7_col120),
            eval!(context, value_limb_8_col121),
            eval!(context, value_limb_9_col122),
            eval!(context, value_limb_10_col123),
            eval!(context, value_limb_11_col124),
            eval!(context, value_limb_12_col125),
            eval!(context, value_limb_13_col126),
            eval!(context, value_limb_14_col127),
            eval!(context, value_limb_15_col128),
            eval!(context, value_limb_16_col129),
            eval!(context, value_limb_17_col130),
            eval!(context, value_limb_18_col131),
            eval!(context, value_limb_19_col132),
            eval!(context, value_limb_20_col133),
            eval!(context, value_limb_21_col134),
            eval!(context, value_limb_22_col135),
            eval!(context, partial_limb_msb_col136),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_5_col6),
            eval!(context, 1),
            eval!(context, value_limb_0_col137),
            eval!(context, value_limb_1_col138),
            eval!(context, value_limb_2_col139),
            eval!(context, value_limb_3_col140),
            eval!(context, value_limb_4_col141),
            eval!(context, value_limb_5_col142),
            eval!(context, value_limb_6_col143),
            eval!(context, value_limb_7_col144),
            eval!(context, value_limb_8_col145),
            eval!(context, value_limb_9_col146),
            eval!(context, value_limb_10_col147),
            eval!(context, value_limb_11_col148),
            eval!(context, value_limb_12_col149),
            eval!(context, value_limb_13_col150),
            eval!(context, value_limb_14_col151),
            eval!(context, value_limb_15_col152),
            eval!(context, value_limb_16_col153),
            eval!(context, value_limb_17_col154),
            eval!(context, value_limb_18_col155),
            eval!(context, value_limb_19_col156),
            eval!(context, value_limb_20_col157),
            eval!(context, value_limb_21_col158),
            eval!(context, value_limb_22_col159),
            eval!(context, partial_limb_msb_col160),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_6_col7),
            eval!(context, 1),
            eval!(context, value_limb_0_col161),
            eval!(context, value_limb_1_col162),
            eval!(context, value_limb_2_col163),
            eval!(context, value_limb_3_col164),
            eval!(context, value_limb_4_col165),
            eval!(context, value_limb_5_col166),
            eval!(context, value_limb_6_col167),
            eval!(context, value_limb_7_col168),
            eval!(context, value_limb_8_col169),
            eval!(context, value_limb_9_col170),
            eval!(context, value_limb_10_col171),
            eval!(context, value_limb_11_col172),
            eval!(context, value_limb_12_col173),
            eval!(context, value_limb_13_col174),
            eval!(context, value_limb_14_col175),
            eval!(context, value_limb_15_col176),
            eval!(context, value_limb_16_col177),
            eval!(context, value_limb_17_col178),
            eval!(context, value_limb_18_col179),
            eval!(context, value_limb_19_col180),
            eval!(context, value_limb_20_col181),
            eval!(context, value_limb_21_col182),
            eval!(context, value_limb_22_col183),
            eval!(context, partial_limb_msb_col184),
        ],
        context,
        component_data,
        acc,
    );

    read_positive_known_id_num_bits_200::accumulate_constraints(
        &[
            eval!(context, input_limb_7_col8),
            eval!(context, 1),
            eval!(context, value_limb_0_col185),
            eval!(context, value_limb_1_col186),
            eval!(context, value_limb_2_col187),
            eval!(context, value_limb_3_col188),
            eval!(context, value_limb_4_col189),
            eval!(context, value_limb_5_col190),
            eval!(context, value_limb_6_col191),
            eval!(context, value_limb_7_col192),
            eval!(context, value_limb_8_col193),
            eval!(context, value_limb_9_col194),
            eval!(context, value_limb_10_col195),
            eval!(context, value_limb_11_col196),
            eval!(context, value_limb_12_col197),
            eval!(context, value_limb_13_col198),
            eval!(context, value_limb_14_col199),
            eval!(context, value_limb_15_col200),
            eval!(context, value_limb_16_col201),
            eval!(context, value_limb_17_col202),
            eval!(context, value_limb_18_col203),
            eval!(context, value_limb_19_col204),
            eval!(context, value_limb_20_col205),
            eval!(context, value_limb_21_col206),
            eval!(context, value_limb_22_col207),
            eval!(context, partial_limb_msb_col208),
        ],
        context,
        component_data,
        acc,
    );

    // Yield KeccakRound.
    let tuple_8 = &[
        eval!(context, 384052509),
        eval!(context, seq),
        eval!(context, 0),
        eval!(
            context,
            ((value_limb_0_col17) + ((512) * (value_limb_1_col18)))
                + ((262144) * (value_limb_2_col19))
        ),
        eval!(
            context,
            ((value_limb_3_col20) + ((512) * (value_limb_4_col21)))
                + ((262144) * (value_limb_5_col22))
        ),
        eval!(
            context,
            ((value_limb_6_col23) + ((512) * (value_limb_7_col24)))
                + ((262144) * (value_limb_8_col25))
        ),
        eval!(
            context,
            ((value_limb_9_col26) + ((512) * (value_limb_10_col27)))
                + ((262144) * (value_limb_11_col28))
        ),
        eval!(
            context,
            ((value_limb_12_col29) + ((512) * (value_limb_13_col30)))
                + ((262144) * (value_limb_14_col31))
        ),
        eval!(
            context,
            ((value_limb_15_col32) + ((512) * (value_limb_16_col33)))
                + ((262144) * (value_limb_17_col34))
        ),
        eval!(
            context,
            ((value_limb_18_col35) + ((512) * (value_limb_19_col36)))
                + ((262144) * (value_limb_20_col37))
        ),
        eval!(context, (value_limb_21_col38) + ((512) * (value_limb_22_col39))),
        eval!(
            context,
            ((value_limb_0_col41) + ((512) * (value_limb_1_col42)))
                + ((262144) * (value_limb_2_col43))
        ),
        eval!(
            context,
            ((value_limb_3_col44) + ((512) * (value_limb_4_col45)))
                + ((262144) * (value_limb_5_col46))
        ),
        eval!(
            context,
            ((value_limb_6_col47) + ((512) * (value_limb_7_col48)))
                + ((262144) * (value_limb_8_col49))
        ),
        eval!(
            context,
            ((value_limb_9_col50) + ((512) * (value_limb_10_col51)))
                + ((262144) * (value_limb_11_col52))
        ),
        eval!(
            context,
            ((value_limb_12_col53) + ((512) * (value_limb_13_col54)))
                + ((262144) * (value_limb_14_col55))
        ),
        eval!(
            context,
            ((value_limb_15_col56) + ((512) * (value_limb_16_col57)))
                + ((262144) * (value_limb_17_col58))
        ),
        eval!(
            context,
            ((value_limb_18_col59) + ((512) * (value_limb_19_col60)))
                + ((262144) * (value_limb_20_col61))
        ),
        eval!(context, (value_limb_21_col62) + ((512) * (value_limb_22_col63))),
        eval!(
            context,
            ((value_limb_0_col65) + ((512) * (value_limb_1_col66)))
                + ((262144) * (value_limb_2_col67))
        ),
        eval!(
            context,
            ((value_limb_3_col68) + ((512) * (value_limb_4_col69)))
                + ((262144) * (value_limb_5_col70))
        ),
        eval!(
            context,
            ((value_limb_6_col71) + ((512) * (value_limb_7_col72)))
                + ((262144) * (value_limb_8_col73))
        ),
        eval!(
            context,
            ((value_limb_9_col74) + ((512) * (value_limb_10_col75)))
                + ((262144) * (value_limb_11_col76))
        ),
        eval!(
            context,
            ((value_limb_12_col77) + ((512) * (value_limb_13_col78)))
                + ((262144) * (value_limb_14_col79))
        ),
        eval!(
            context,
            ((value_limb_15_col80) + ((512) * (value_limb_16_col81)))
                + ((262144) * (value_limb_17_col82))
        ),
        eval!(
            context,
            ((value_limb_18_col83) + ((512) * (value_limb_19_col84)))
                + ((262144) * (value_limb_20_col85))
        ),
        eval!(context, (value_limb_21_col86) + ((512) * (value_limb_22_col87))),
        eval!(
            context,
            ((value_limb_0_col89) + ((512) * (value_limb_1_col90)))
                + ((262144) * (value_limb_2_col91))
        ),
        eval!(
            context,
            ((value_limb_3_col92) + ((512) * (value_limb_4_col93)))
                + ((262144) * (value_limb_5_col94))
        ),
        eval!(
            context,
            ((value_limb_6_col95) + ((512) * (value_limb_7_col96)))
                + ((262144) * (value_limb_8_col97))
        ),
        eval!(
            context,
            ((value_limb_9_col98) + ((512) * (value_limb_10_col99)))
                + ((262144) * (value_limb_11_col100))
        ),
        eval!(
            context,
            ((value_limb_12_col101) + ((512) * (value_limb_13_col102)))
                + ((262144) * (value_limb_14_col103))
        ),
        eval!(
            context,
            ((value_limb_15_col104) + ((512) * (value_limb_16_col105)))
                + ((262144) * (value_limb_17_col106))
        ),
        eval!(
            context,
            ((value_limb_18_col107) + ((512) * (value_limb_19_col108)))
                + ((262144) * (value_limb_20_col109))
        ),
        eval!(context, (value_limb_21_col110) + ((512) * (value_limb_22_col111))),
        eval!(
            context,
            ((value_limb_0_col113) + ((512) * (value_limb_1_col114)))
                + ((262144) * (value_limb_2_col115))
        ),
        eval!(
            context,
            ((value_limb_3_col116) + ((512) * (value_limb_4_col117)))
                + ((262144) * (value_limb_5_col118))
        ),
        eval!(
            context,
            ((value_limb_6_col119) + ((512) * (value_limb_7_col120)))
                + ((262144) * (value_limb_8_col121))
        ),
        eval!(
            context,
            ((value_limb_9_col122) + ((512) * (value_limb_10_col123)))
                + ((262144) * (value_limb_11_col124))
        ),
        eval!(
            context,
            ((value_limb_12_col125) + ((512) * (value_limb_13_col126)))
                + ((262144) * (value_limb_14_col127))
        ),
        eval!(
            context,
            ((value_limb_15_col128) + ((512) * (value_limb_16_col129)))
                + ((262144) * (value_limb_17_col130))
        ),
        eval!(
            context,
            ((value_limb_18_col131) + ((512) * (value_limb_19_col132)))
                + ((262144) * (value_limb_20_col133))
        ),
        eval!(context, (value_limb_21_col134) + ((512) * (value_limb_22_col135))),
        eval!(
            context,
            ((value_limb_0_col137) + ((512) * (value_limb_1_col138)))
                + ((262144) * (value_limb_2_col139))
        ),
        eval!(
            context,
            ((value_limb_3_col140) + ((512) * (value_limb_4_col141)))
                + ((262144) * (value_limb_5_col142))
        ),
        eval!(
            context,
            ((value_limb_6_col143) + ((512) * (value_limb_7_col144)))
                + ((262144) * (value_limb_8_col145))
        ),
        eval!(
            context,
            ((value_limb_9_col146) + ((512) * (value_limb_10_col147)))
                + ((262144) * (value_limb_11_col148))
        ),
        eval!(
            context,
            ((value_limb_12_col149) + ((512) * (value_limb_13_col150)))
                + ((262144) * (value_limb_14_col151))
        ),
        eval!(
            context,
            ((value_limb_15_col152) + ((512) * (value_limb_16_col153)))
                + ((262144) * (value_limb_17_col154))
        ),
        eval!(
            context,
            ((value_limb_18_col155) + ((512) * (value_limb_19_col156)))
                + ((262144) * (value_limb_20_col157))
        ),
        eval!(context, (value_limb_21_col158) + ((512) * (value_limb_22_col159))),
        eval!(
            context,
            ((value_limb_0_col161) + ((512) * (value_limb_1_col162)))
                + ((262144) * (value_limb_2_col163))
        ),
        eval!(
            context,
            ((value_limb_3_col164) + ((512) * (value_limb_4_col165)))
                + ((262144) * (value_limb_5_col166))
        ),
        eval!(
            context,
            ((value_limb_6_col167) + ((512) * (value_limb_7_col168)))
                + ((262144) * (value_limb_8_col169))
        ),
        eval!(
            context,
            ((value_limb_9_col170) + ((512) * (value_limb_10_col171)))
                + ((262144) * (value_limb_11_col172))
        ),
        eval!(
            context,
            ((value_limb_12_col173) + ((512) * (value_limb_13_col174)))
                + ((262144) * (value_limb_14_col175))
        ),
        eval!(
            context,
            ((value_limb_15_col176) + ((512) * (value_limb_16_col177)))
                + ((262144) * (value_limb_17_col178))
        ),
        eval!(
            context,
            ((value_limb_18_col179) + ((512) * (value_limb_19_col180)))
                + ((262144) * (value_limb_20_col181))
        ),
        eval!(context, (value_limb_21_col182) + ((512) * (value_limb_22_col183))),
        eval!(
            context,
            ((value_limb_0_col185) + ((512) * (value_limb_1_col186)))
                + ((262144) * (value_limb_2_col187))
        ),
        eval!(
            context,
            ((value_limb_3_col188) + ((512) * (value_limb_4_col189)))
                + ((262144) * (value_limb_5_col190))
        ),
        eval!(
            context,
            ((value_limb_6_col191) + ((512) * (value_limb_7_col192)))
                + ((262144) * (value_limb_8_col193))
        ),
        eval!(
            context,
            ((value_limb_9_col194) + ((512) * (value_limb_10_col195)))
                + ((262144) * (value_limb_11_col196))
        ),
        eval!(
            context,
            ((value_limb_12_col197) + ((512) * (value_limb_13_col198)))
                + ((262144) * (value_limb_14_col199))
        ),
        eval!(
            context,
            ((value_limb_15_col200) + ((512) * (value_limb_16_col201)))
                + ((262144) * (value_limb_17_col202))
        ),
        eval!(
            context,
            ((value_limb_18_col203) + ((512) * (value_limb_19_col204)))
                + ((262144) * (value_limb_20_col205))
        ),
        eval!(context, (value_limb_21_col206) + ((512) * (value_limb_22_col207))),
    ];
    let numerator_8 = eval!(context, -(1));
    acc.add_to_relation(context, numerator_8, tuple_8);

    // Use KeccakRound.
    let tuple_9 = &[
        eval!(context, 384052509),
        eval!(context, seq),
        eval!(context, 24),
        eval!(context, keccak_round_output_limb_0_col209),
        eval!(context, keccak_round_output_limb_1_col210),
        eval!(context, keccak_round_output_limb_2_col211),
        eval!(context, keccak_round_output_limb_3_col212),
        eval!(context, keccak_round_output_limb_4_col213),
        eval!(context, keccak_round_output_limb_5_col214),
        eval!(context, keccak_round_output_limb_6_col215),
        eval!(context, keccak_round_output_limb_7_col216),
        eval!(context, keccak_round_output_limb_8_col217),
        eval!(context, keccak_round_output_limb_9_col218),
        eval!(context, keccak_round_output_limb_10_col219),
        eval!(context, keccak_round_output_limb_11_col220),
        eval!(context, keccak_round_output_limb_12_col221),
        eval!(context, keccak_round_output_limb_13_col222),
        eval!(context, keccak_round_output_limb_14_col223),
        eval!(context, keccak_round_output_limb_15_col224),
        eval!(context, keccak_round_output_limb_16_col225),
        eval!(context, keccak_round_output_limb_17_col226),
        eval!(context, keccak_round_output_limb_18_col227),
        eval!(context, keccak_round_output_limb_19_col228),
        eval!(context, keccak_round_output_limb_20_col229),
        eval!(context, keccak_round_output_limb_21_col230),
        eval!(context, keccak_round_output_limb_22_col231),
        eval!(context, keccak_round_output_limb_23_col232),
        eval!(context, keccak_round_output_limb_24_col233),
        eval!(context, keccak_round_output_limb_25_col234),
        eval!(context, keccak_round_output_limb_26_col235),
        eval!(context, keccak_round_output_limb_27_col236),
        eval!(context, keccak_round_output_limb_28_col237),
        eval!(context, keccak_round_output_limb_29_col238),
        eval!(context, keccak_round_output_limb_30_col239),
        eval!(context, keccak_round_output_limb_31_col240),
        eval!(context, keccak_round_output_limb_32_col241),
        eval!(context, keccak_round_output_limb_33_col242),
        eval!(context, keccak_round_output_limb_34_col243),
        eval!(context, keccak_round_output_limb_35_col244),
        eval!(context, keccak_round_output_limb_36_col245),
        eval!(context, keccak_round_output_limb_37_col246),
        eval!(context, keccak_round_output_limb_38_col247),
        eval!(context, keccak_round_output_limb_39_col248),
        eval!(context, keccak_round_output_limb_40_col249),
        eval!(context, keccak_round_output_limb_41_col250),
        eval!(context, keccak_round_output_limb_42_col251),
        eval!(context, keccak_round_output_limb_43_col252),
        eval!(context, keccak_round_output_limb_44_col253),
        eval!(context, keccak_round_output_limb_45_col254),
        eval!(context, keccak_round_output_limb_46_col255),
        eval!(context, keccak_round_output_limb_47_col256),
        eval!(context, keccak_round_output_limb_48_col257),
        eval!(context, keccak_round_output_limb_49_col258),
        eval!(context, keccak_round_output_limb_50_col259),
        eval!(context, keccak_round_output_limb_51_col260),
        eval!(context, keccak_round_output_limb_52_col261),
        eval!(context, keccak_round_output_limb_53_col262),
        eval!(context, keccak_round_output_limb_54_col263),
        eval!(context, keccak_round_output_limb_55_col264),
        eval!(context, keccak_round_output_limb_56_col265),
        eval!(context, keccak_round_output_limb_57_col266),
        eval!(context, keccak_round_output_limb_58_col267),
        eval!(context, keccak_round_output_limb_59_col268),
        eval!(context, keccak_round_output_limb_60_col269),
        eval!(context, keccak_round_output_limb_61_col270),
        eval!(context, keccak_round_output_limb_62_col271),
        eval!(context, keccak_round_output_limb_63_col272),
    ];
    let numerator_9 = eval!(context, 1);
    acc.add_to_relation(context, numerator_9, tuple_9);

    let output_memory_limb_last_tmp_8d1be_59 = eval!(
        context,
        (((keccak_round_output_limb_0_col209) - (output_memory_limb_col273))
            - ((512) * (output_memory_limb_col274)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_62 = eval!(
        context,
        (((keccak_round_output_limb_1_col210) - (output_memory_limb_col275))
            - ((512) * (output_memory_limb_col276)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_65 = eval!(
        context,
        (((keccak_round_output_limb_2_col211) - (output_memory_limb_col277))
            - ((512) * (output_memory_limb_col278)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_68 = eval!(
        context,
        (((keccak_round_output_limb_3_col212) - (output_memory_limb_col279))
            - ((512) * (output_memory_limb_col280)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_71 = eval!(
        context,
        (((keccak_round_output_limb_4_col213) - (output_memory_limb_col281))
            - ((512) * (output_memory_limb_col282)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_74 = eval!(
        context,
        (((keccak_round_output_limb_5_col214) - (output_memory_limb_col283))
            - ((512) * (output_memory_limb_col284)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_77 = eval!(
        context,
        (((keccak_round_output_limb_6_col215) - (output_memory_limb_col285))
            - ((512) * (output_memory_limb_col286)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_79 = eval!(
        context,
        ((keccak_round_output_limb_7_col216) - (output_memory_limb_col287)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_18 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_8_col9),
        eval!(context, output_memory_limb_col273),
        eval!(context, output_memory_limb_col274),
        eval!(context, output_memory_limb_last_tmp_8d1be_59),
        eval!(context, output_memory_limb_col275),
        eval!(context, output_memory_limb_col276),
        eval!(context, output_memory_limb_last_tmp_8d1be_62),
        eval!(context, output_memory_limb_col277),
        eval!(context, output_memory_limb_col278),
        eval!(context, output_memory_limb_last_tmp_8d1be_65),
        eval!(context, output_memory_limb_col279),
        eval!(context, output_memory_limb_col280),
        eval!(context, output_memory_limb_last_tmp_8d1be_68),
        eval!(context, output_memory_limb_col281),
        eval!(context, output_memory_limb_col282),
        eval!(context, output_memory_limb_last_tmp_8d1be_71),
        eval!(context, output_memory_limb_col283),
        eval!(context, output_memory_limb_col284),
        eval!(context, output_memory_limb_last_tmp_8d1be_74),
        eval!(context, output_memory_limb_col285),
        eval!(context, output_memory_limb_col286),
        eval!(context, output_memory_limb_last_tmp_8d1be_77),
        eval!(context, output_memory_limb_col287),
        eval!(context, output_memory_limb_last_tmp_8d1be_79),
    ];
    let numerator_18 = eval!(context, 1);
    acc.add_to_relation(context, numerator_18, tuple_18);

    let output_memory_limb_last_tmp_8d1be_82 = eval!(
        context,
        (((keccak_round_output_limb_8_col217) - (output_memory_limb_col288))
            - ((512) * (output_memory_limb_col289)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_85 = eval!(
        context,
        (((keccak_round_output_limb_9_col218) - (output_memory_limb_col290))
            - ((512) * (output_memory_limb_col291)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_88 = eval!(
        context,
        (((keccak_round_output_limb_10_col219) - (output_memory_limb_col292))
            - ((512) * (output_memory_limb_col293)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_91 = eval!(
        context,
        (((keccak_round_output_limb_11_col220) - (output_memory_limb_col294))
            - ((512) * (output_memory_limb_col295)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_94 = eval!(
        context,
        (((keccak_round_output_limb_12_col221) - (output_memory_limb_col296))
            - ((512) * (output_memory_limb_col297)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_97 = eval!(
        context,
        (((keccak_round_output_limb_13_col222) - (output_memory_limb_col298))
            - ((512) * (output_memory_limb_col299)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_100 = eval!(
        context,
        (((keccak_round_output_limb_14_col223) - (output_memory_limb_col300))
            - ((512) * (output_memory_limb_col301)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_102 = eval!(
        context,
        ((keccak_round_output_limb_15_col224) - (output_memory_limb_col302)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_27 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_9_col10),
        eval!(context, output_memory_limb_col288),
        eval!(context, output_memory_limb_col289),
        eval!(context, output_memory_limb_last_tmp_8d1be_82),
        eval!(context, output_memory_limb_col290),
        eval!(context, output_memory_limb_col291),
        eval!(context, output_memory_limb_last_tmp_8d1be_85),
        eval!(context, output_memory_limb_col292),
        eval!(context, output_memory_limb_col293),
        eval!(context, output_memory_limb_last_tmp_8d1be_88),
        eval!(context, output_memory_limb_col294),
        eval!(context, output_memory_limb_col295),
        eval!(context, output_memory_limb_last_tmp_8d1be_91),
        eval!(context, output_memory_limb_col296),
        eval!(context, output_memory_limb_col297),
        eval!(context, output_memory_limb_last_tmp_8d1be_94),
        eval!(context, output_memory_limb_col298),
        eval!(context, output_memory_limb_col299),
        eval!(context, output_memory_limb_last_tmp_8d1be_97),
        eval!(context, output_memory_limb_col300),
        eval!(context, output_memory_limb_col301),
        eval!(context, output_memory_limb_last_tmp_8d1be_100),
        eval!(context, output_memory_limb_col302),
        eval!(context, output_memory_limb_last_tmp_8d1be_102),
    ];
    let numerator_27 = eval!(context, 1);
    acc.add_to_relation(context, numerator_27, tuple_27);

    let output_memory_limb_last_tmp_8d1be_105 = eval!(
        context,
        (((keccak_round_output_limb_16_col225) - (output_memory_limb_col303))
            - ((512) * (output_memory_limb_col304)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_108 = eval!(
        context,
        (((keccak_round_output_limb_17_col226) - (output_memory_limb_col305))
            - ((512) * (output_memory_limb_col306)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_111 = eval!(
        context,
        (((keccak_round_output_limb_18_col227) - (output_memory_limb_col307))
            - ((512) * (output_memory_limb_col308)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_114 = eval!(
        context,
        (((keccak_round_output_limb_19_col228) - (output_memory_limb_col309))
            - ((512) * (output_memory_limb_col310)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_117 = eval!(
        context,
        (((keccak_round_output_limb_20_col229) - (output_memory_limb_col311))
            - ((512) * (output_memory_limb_col312)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_120 = eval!(
        context,
        (((keccak_round_output_limb_21_col230) - (output_memory_limb_col313))
            - ((512) * (output_memory_limb_col314)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_123 = eval!(
        context,
        (((keccak_round_output_limb_22_col231) - (output_memory_limb_col315))
            - ((512) * (output_memory_limb_col316)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_125 = eval!(
        context,
        ((keccak_round_output_limb_23_col232) - (output_memory_limb_col317)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_36 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_10_col11),
        eval!(context, output_memory_limb_col303),
        eval!(context, output_memory_limb_col304),
        eval!(context, output_memory_limb_last_tmp_8d1be_105),
        eval!(context, output_memory_limb_col305),
        eval!(context, output_memory_limb_col306),
        eval!(context, output_memory_limb_last_tmp_8d1be_108),
        eval!(context, output_memory_limb_col307),
        eval!(context, output_memory_limb_col308),
        eval!(context, output_memory_limb_last_tmp_8d1be_111),
        eval!(context, output_memory_limb_col309),
        eval!(context, output_memory_limb_col310),
        eval!(context, output_memory_limb_last_tmp_8d1be_114),
        eval!(context, output_memory_limb_col311),
        eval!(context, output_memory_limb_col312),
        eval!(context, output_memory_limb_last_tmp_8d1be_117),
        eval!(context, output_memory_limb_col313),
        eval!(context, output_memory_limb_col314),
        eval!(context, output_memory_limb_last_tmp_8d1be_120),
        eval!(context, output_memory_limb_col315),
        eval!(context, output_memory_limb_col316),
        eval!(context, output_memory_limb_last_tmp_8d1be_123),
        eval!(context, output_memory_limb_col317),
        eval!(context, output_memory_limb_last_tmp_8d1be_125),
    ];
    let numerator_36 = eval!(context, 1);
    acc.add_to_relation(context, numerator_36, tuple_36);

    let output_memory_limb_last_tmp_8d1be_128 = eval!(
        context,
        (((keccak_round_output_limb_24_col233) - (output_memory_limb_col318))
            - ((512) * (output_memory_limb_col319)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_131 = eval!(
        context,
        (((keccak_round_output_limb_25_col234) - (output_memory_limb_col320))
            - ((512) * (output_memory_limb_col321)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_134 = eval!(
        context,
        (((keccak_round_output_limb_26_col235) - (output_memory_limb_col322))
            - ((512) * (output_memory_limb_col323)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_137 = eval!(
        context,
        (((keccak_round_output_limb_27_col236) - (output_memory_limb_col324))
            - ((512) * (output_memory_limb_col325)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_140 = eval!(
        context,
        (((keccak_round_output_limb_28_col237) - (output_memory_limb_col326))
            - ((512) * (output_memory_limb_col327)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_143 = eval!(
        context,
        (((keccak_round_output_limb_29_col238) - (output_memory_limb_col328))
            - ((512) * (output_memory_limb_col329)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_146 = eval!(
        context,
        (((keccak_round_output_limb_30_col239) - (output_memory_limb_col330))
            - ((512) * (output_memory_limb_col331)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_148 = eval!(
        context,
        ((keccak_round_output_limb_31_col240) - (output_memory_limb_col332)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_45 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_11_col12),
        eval!(context, output_memory_limb_col318),
        eval!(context, output_memory_limb_col319),
        eval!(context, output_memory_limb_last_tmp_8d1be_128),
        eval!(context, output_memory_limb_col320),
        eval!(context, output_memory_limb_col321),
        eval!(context, output_memory_limb_last_tmp_8d1be_131),
        eval!(context, output_memory_limb_col322),
        eval!(context, output_memory_limb_col323),
        eval!(context, output_memory_limb_last_tmp_8d1be_134),
        eval!(context, output_memory_limb_col324),
        eval!(context, output_memory_limb_col325),
        eval!(context, output_memory_limb_last_tmp_8d1be_137),
        eval!(context, output_memory_limb_col326),
        eval!(context, output_memory_limb_col327),
        eval!(context, output_memory_limb_last_tmp_8d1be_140),
        eval!(context, output_memory_limb_col328),
        eval!(context, output_memory_limb_col329),
        eval!(context, output_memory_limb_last_tmp_8d1be_143),
        eval!(context, output_memory_limb_col330),
        eval!(context, output_memory_limb_col331),
        eval!(context, output_memory_limb_last_tmp_8d1be_146),
        eval!(context, output_memory_limb_col332),
        eval!(context, output_memory_limb_last_tmp_8d1be_148),
    ];
    let numerator_45 = eval!(context, 1);
    acc.add_to_relation(context, numerator_45, tuple_45);

    let output_memory_limb_last_tmp_8d1be_151 = eval!(
        context,
        (((keccak_round_output_limb_32_col241) - (output_memory_limb_col333))
            - ((512) * (output_memory_limb_col334)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_154 = eval!(
        context,
        (((keccak_round_output_limb_33_col242) - (output_memory_limb_col335))
            - ((512) * (output_memory_limb_col336)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_157 = eval!(
        context,
        (((keccak_round_output_limb_34_col243) - (output_memory_limb_col337))
            - ((512) * (output_memory_limb_col338)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_160 = eval!(
        context,
        (((keccak_round_output_limb_35_col244) - (output_memory_limb_col339))
            - ((512) * (output_memory_limb_col340)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_163 = eval!(
        context,
        (((keccak_round_output_limb_36_col245) - (output_memory_limb_col341))
            - ((512) * (output_memory_limb_col342)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_166 = eval!(
        context,
        (((keccak_round_output_limb_37_col246) - (output_memory_limb_col343))
            - ((512) * (output_memory_limb_col344)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_169 = eval!(
        context,
        (((keccak_round_output_limb_38_col247) - (output_memory_limb_col345))
            - ((512) * (output_memory_limb_col346)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_171 = eval!(
        context,
        ((keccak_round_output_limb_39_col248) - (output_memory_limb_col347)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_54 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_12_col13),
        eval!(context, output_memory_limb_col333),
        eval!(context, output_memory_limb_col334),
        eval!(context, output_memory_limb_last_tmp_8d1be_151),
        eval!(context, output_memory_limb_col335),
        eval!(context, output_memory_limb_col336),
        eval!(context, output_memory_limb_last_tmp_8d1be_154),
        eval!(context, output_memory_limb_col337),
        eval!(context, output_memory_limb_col338),
        eval!(context, output_memory_limb_last_tmp_8d1be_157),
        eval!(context, output_memory_limb_col339),
        eval!(context, output_memory_limb_col340),
        eval!(context, output_memory_limb_last_tmp_8d1be_160),
        eval!(context, output_memory_limb_col341),
        eval!(context, output_memory_limb_col342),
        eval!(context, output_memory_limb_last_tmp_8d1be_163),
        eval!(context, output_memory_limb_col343),
        eval!(context, output_memory_limb_col344),
        eval!(context, output_memory_limb_last_tmp_8d1be_166),
        eval!(context, output_memory_limb_col345),
        eval!(context, output_memory_limb_col346),
        eval!(context, output_memory_limb_last_tmp_8d1be_169),
        eval!(context, output_memory_limb_col347),
        eval!(context, output_memory_limb_last_tmp_8d1be_171),
    ];
    let numerator_54 = eval!(context, 1);
    acc.add_to_relation(context, numerator_54, tuple_54);

    let output_memory_limb_last_tmp_8d1be_174 = eval!(
        context,
        (((keccak_round_output_limb_40_col249) - (output_memory_limb_col348))
            - ((512) * (output_memory_limb_col349)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_177 = eval!(
        context,
        (((keccak_round_output_limb_41_col250) - (output_memory_limb_col350))
            - ((512) * (output_memory_limb_col351)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_180 = eval!(
        context,
        (((keccak_round_output_limb_42_col251) - (output_memory_limb_col352))
            - ((512) * (output_memory_limb_col353)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_183 = eval!(
        context,
        (((keccak_round_output_limb_43_col252) - (output_memory_limb_col354))
            - ((512) * (output_memory_limb_col355)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_186 = eval!(
        context,
        (((keccak_round_output_limb_44_col253) - (output_memory_limb_col356))
            - ((512) * (output_memory_limb_col357)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_189 = eval!(
        context,
        (((keccak_round_output_limb_45_col254) - (output_memory_limb_col358))
            - ((512) * (output_memory_limb_col359)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_192 = eval!(
        context,
        (((keccak_round_output_limb_46_col255) - (output_memory_limb_col360))
            - ((512) * (output_memory_limb_col361)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_194 = eval!(
        context,
        ((keccak_round_output_limb_47_col256) - (output_memory_limb_col362)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_63 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_13_col14),
        eval!(context, output_memory_limb_col348),
        eval!(context, output_memory_limb_col349),
        eval!(context, output_memory_limb_last_tmp_8d1be_174),
        eval!(context, output_memory_limb_col350),
        eval!(context, output_memory_limb_col351),
        eval!(context, output_memory_limb_last_tmp_8d1be_177),
        eval!(context, output_memory_limb_col352),
        eval!(context, output_memory_limb_col353),
        eval!(context, output_memory_limb_last_tmp_8d1be_180),
        eval!(context, output_memory_limb_col354),
        eval!(context, output_memory_limb_col355),
        eval!(context, output_memory_limb_last_tmp_8d1be_183),
        eval!(context, output_memory_limb_col356),
        eval!(context, output_memory_limb_col357),
        eval!(context, output_memory_limb_last_tmp_8d1be_186),
        eval!(context, output_memory_limb_col358),
        eval!(context, output_memory_limb_col359),
        eval!(context, output_memory_limb_last_tmp_8d1be_189),
        eval!(context, output_memory_limb_col360),
        eval!(context, output_memory_limb_col361),
        eval!(context, output_memory_limb_last_tmp_8d1be_192),
        eval!(context, output_memory_limb_col362),
        eval!(context, output_memory_limb_last_tmp_8d1be_194),
    ];
    let numerator_63 = eval!(context, 1);
    acc.add_to_relation(context, numerator_63, tuple_63);

    let output_memory_limb_last_tmp_8d1be_197 = eval!(
        context,
        (((keccak_round_output_limb_48_col257) - (output_memory_limb_col363))
            - ((512) * (output_memory_limb_col364)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_200 = eval!(
        context,
        (((keccak_round_output_limb_49_col258) - (output_memory_limb_col365))
            - ((512) * (output_memory_limb_col366)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_203 = eval!(
        context,
        (((keccak_round_output_limb_50_col259) - (output_memory_limb_col367))
            - ((512) * (output_memory_limb_col368)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_206 = eval!(
        context,
        (((keccak_round_output_limb_51_col260) - (output_memory_limb_col369))
            - ((512) * (output_memory_limb_col370)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_209 = eval!(
        context,
        (((keccak_round_output_limb_52_col261) - (output_memory_limb_col371))
            - ((512) * (output_memory_limb_col372)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_212 = eval!(
        context,
        (((keccak_round_output_limb_53_col262) - (output_memory_limb_col373))
            - ((512) * (output_memory_limb_col374)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_215 = eval!(
        context,
        (((keccak_round_output_limb_54_col263) - (output_memory_limb_col375))
            - ((512) * (output_memory_limb_col376)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_217 = eval!(
        context,
        ((keccak_round_output_limb_55_col264) - (output_memory_limb_col377)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_72 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_14_col15),
        eval!(context, output_memory_limb_col363),
        eval!(context, output_memory_limb_col364),
        eval!(context, output_memory_limb_last_tmp_8d1be_197),
        eval!(context, output_memory_limb_col365),
        eval!(context, output_memory_limb_col366),
        eval!(context, output_memory_limb_last_tmp_8d1be_200),
        eval!(context, output_memory_limb_col367),
        eval!(context, output_memory_limb_col368),
        eval!(context, output_memory_limb_last_tmp_8d1be_203),
        eval!(context, output_memory_limb_col369),
        eval!(context, output_memory_limb_col370),
        eval!(context, output_memory_limb_last_tmp_8d1be_206),
        eval!(context, output_memory_limb_col371),
        eval!(context, output_memory_limb_col372),
        eval!(context, output_memory_limb_last_tmp_8d1be_209),
        eval!(context, output_memory_limb_col373),
        eval!(context, output_memory_limb_col374),
        eval!(context, output_memory_limb_last_tmp_8d1be_212),
        eval!(context, output_memory_limb_col375),
        eval!(context, output_memory_limb_col376),
        eval!(context, output_memory_limb_last_tmp_8d1be_215),
        eval!(context, output_memory_limb_col377),
        eval!(context, output_memory_limb_last_tmp_8d1be_217),
    ];
    let numerator_72 = eval!(context, 1);
    acc.add_to_relation(context, numerator_72, tuple_72);

    let output_memory_limb_last_tmp_8d1be_220 = eval!(
        context,
        (((keccak_round_output_limb_56_col265) - (output_memory_limb_col378))
            - ((512) * (output_memory_limb_col379)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_223 = eval!(
        context,
        (((keccak_round_output_limb_57_col266) - (output_memory_limb_col380))
            - ((512) * (output_memory_limb_col381)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_226 = eval!(
        context,
        (((keccak_round_output_limb_58_col267) - (output_memory_limb_col382))
            - ((512) * (output_memory_limb_col383)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_229 = eval!(
        context,
        (((keccak_round_output_limb_59_col268) - (output_memory_limb_col384))
            - ((512) * (output_memory_limb_col385)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_232 = eval!(
        context,
        (((keccak_round_output_limb_60_col269) - (output_memory_limb_col386))
            - ((512) * (output_memory_limb_col387)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_235 = eval!(
        context,
        (((keccak_round_output_limb_61_col270) - (output_memory_limb_col388))
            - ((512) * (output_memory_limb_col389)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_238 = eval!(
        context,
        (((keccak_round_output_limb_62_col271) - (output_memory_limb_col390))
            - ((512) * (output_memory_limb_col391)))
            * (8192)
    );

    let output_memory_limb_last_tmp_8d1be_240 = eval!(
        context,
        ((keccak_round_output_limb_63_col272) - (output_memory_limb_col392)) * (4194304)
    );

    // Use MemoryIdToBig.
    let tuple_81 = &[
        eval!(context, 1662111297),
        eval!(context, input_limb_15_col16),
        eval!(context, output_memory_limb_col378),
        eval!(context, output_memory_limb_col379),
        eval!(context, output_memory_limb_last_tmp_8d1be_220),
        eval!(context, output_memory_limb_col380),
        eval!(context, output_memory_limb_col381),
        eval!(context, output_memory_limb_last_tmp_8d1be_223),
        eval!(context, output_memory_limb_col382),
        eval!(context, output_memory_limb_col383),
        eval!(context, output_memory_limb_last_tmp_8d1be_226),
        eval!(context, output_memory_limb_col384),
        eval!(context, output_memory_limb_col385),
        eval!(context, output_memory_limb_last_tmp_8d1be_229),
        eval!(context, output_memory_limb_col386),
        eval!(context, output_memory_limb_col387),
        eval!(context, output_memory_limb_last_tmp_8d1be_232),
        eval!(context, output_memory_limb_col388),
        eval!(context, output_memory_limb_col389),
        eval!(context, output_memory_limb_last_tmp_8d1be_235),
        eval!(context, output_memory_limb_col390),
        eval!(context, output_memory_limb_col391),
        eval!(context, output_memory_limb_last_tmp_8d1be_238),
        eval!(context, output_memory_limb_col392),
        eval!(context, output_memory_limb_last_tmp_8d1be_240),
    ];
    let numerator_81 = eval!(context, 1);
    acc.add_to_relation(context, numerator_81, tuple_81);

    // Yield KeccakAggregator.
    let tuple_82 = &[
        eval!(context, 1973115366),
        eval!(context, input_limb_0_col1),
        eval!(context, input_limb_1_col2),
        eval!(context, input_limb_2_col3),
        eval!(context, input_limb_3_col4),
        eval!(context, input_limb_4_col5),
        eval!(context, input_limb_5_col6),
        eval!(context, input_limb_6_col7),
        eval!(context, input_limb_7_col8),
        eval!(context, input_limb_8_col9),
        eval!(context, input_limb_9_col10),
        eval!(context, input_limb_10_col11),
        eval!(context, input_limb_11_col12),
        eval!(context, input_limb_12_col13),
        eval!(context, input_limb_13_col14),
        eval!(context, input_limb_14_col15),
        eval!(context, input_limb_15_col16),
    ];
    let numerator_82 = eval!(context, -(multiplicity_0_col0));
    acc.add_to_relation(context, numerator_82, tuple_82);
}

pub struct Component {}
impl<Value: IValue> CircuitEval<Value> for Component {
    fn name(&self) -> String {
        "keccak_aggregator".to_string()
    }

    fn evaluate(
        &self,
        context: &mut Context<Value>,
        component_data: &dyn ComponentDataTrait<Value>,
        acc: &mut CompositionConstraintAccumulator,
    ) {
        accumulate_constraints(component_data.trace_columns(), context, component_data, acc);
    }

    fn trace_columns(&self) -> usize {
        N_TRACE_COLUMNS
    }

    fn interaction_columns(&self) -> usize {
        N_INTERACTION_COLUMNS
    }

    fn relation_uses_per_row(&self) -> &[RelationUse] {
        &RELATION_USES_PER_ROW
    }

    fn log_size(
        &self,
        _preprocessed_column_log_sizes: &OrderedHashMap<PreProcessedColumnId, u32>,
    ) -> Option<u32> {
        None
    }
}
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use circuits::context::Context;
    use circuits::ivalue::qm31_from_u32s;
    use circuits_stark_verifier::constraint_eval::*;
    use circuits_stark_verifier::test_utils::TestComponentData;
    use stwo::core::fields::qm31::QM31;

    use super::Component;
    #[allow(unused_imports)]
    use crate::components::prelude::PreProcessedColumnId;
    use crate::sample_evaluations::*;

    #[test]
    fn test_evaluation_result() {
        let component = Component {};
        let mut context: Context<QM31> = Default::default();
        context.enable_assert_eq_on_eval();
        let trace_columns = [
            qm31_from_u32s(1659099300, 905558730, 651199673, 1375009625),
            qm31_from_u32s(1591990121, 771341002, 584090809, 1375009625),
            qm31_from_u32s(1793317658, 1173994186, 785417401, 1375009625),
            qm31_from_u32s(1726208479, 1039776458, 718308537, 1375009625),
            qm31_from_u32s(1390662584, 368687818, 382764217, 1375009625),
            qm31_from_u32s(1323553405, 234470090, 315655353, 1375009625),
            qm31_from_u32s(1524880942, 637123274, 516981945, 1375009625),
            qm31_from_u32s(1457771763, 502905546, 449873081, 1375009625),
            qm31_from_u32s(48489085, 1979300555, 1188070585, 1375009625),
            qm31_from_u32s(2128863553, 1845082826, 1120961721, 1375009625),
            qm31_from_u32s(1852335767, 645078115, 2059236183, 343880121),
            qm31_from_u32s(1919444946, 779295843, 2126345047, 343880121),
            qm31_from_u32s(1986554125, 913513571, 45970264, 343880122),
            qm31_from_u32s(2053663304, 1047731299, 113079128, 343880122),
            qm31_from_u32s(1583899051, 108207203, 1790800727, 343880121),
            qm31_from_u32s(1651008230, 242424931, 1857909591, 343880121),
            qm31_from_u32s(1718117409, 376642659, 1925018455, 343880121),
            qm31_from_u32s(1785226588, 510860387, 1992127319, 343880121),
            qm31_from_u32s(1315462335, 1718819938, 1522365270, 343880121),
            qm31_from_u32s(1382571514, 1853037666, 1589474134, 343880121),
            qm31_from_u32s(1986820986, 913513739, 45970432, 343880178),
            qm31_from_u32s(1919711807, 779296011, 2126345215, 343880177),
            qm31_from_u32s(2121039344, 1181949195, 180188160, 343880178),
            qm31_from_u32s(2053930165, 1047731467, 113079296, 343880178),
            qm31_from_u32s(1718384270, 376642827, 1925018623, 343880177),
            qm31_from_u32s(1651275091, 242425099, 1857909759, 343880177),
            qm31_from_u32s(1852602628, 645078283, 2059236351, 343880177),
            qm31_from_u32s(1785493449, 510860555, 1992127487, 343880177),
            qm31_from_u32s(1449947554, 1987255562, 1656583166, 343880177),
            qm31_from_u32s(1382838375, 1853037834, 1589474302, 343880177),
            qm31_from_u32s(510356977, 108207322, 717059022, 343880161),
            qm31_from_u32s(577466156, 242425050, 784167886, 343880161),
            qm31_from_u32s(376138619, 1987255513, 582841293, 343880161),
            qm31_from_u32s(443247798, 2121473241, 649950157, 343880161),
            qm31_from_u32s(778793693, 645078234, 985494478, 343880161),
            qm31_from_u32s(845902872, 779295962, 1052603342, 343880161),
            qm31_from_u32s(644575335, 376642778, 851276750, 343880161),
            qm31_from_u32s(711684514, 510860506, 918385614, 343880161),
            qm31_from_u32s(1047230409, 1181949146, 1253929934, 343880161),
            qm31_from_u32s(1114339588, 1316166874, 1321038798, 343880161),
            qm31_from_u32s(1717810224, 376642479, 1925018275, 343880061),
            qm31_from_u32s(1650701045, 242424751, 1857909411, 343880061),
            qm31_from_u32s(1583591866, 108207023, 1790800547, 343880061),
            qm31_from_u32s(1516482687, 2121472942, 1723691682, 343880061),
            qm31_from_u32s(1986246940, 913513391, 45970084, 343880062),
            qm31_from_u32s(1919137761, 779295663, 2126344867, 343880061),
            qm31_from_u32s(1852028582, 645077935, 2059236003, 343880061),
            qm31_from_u32s(1784919403, 510860207, 1992127139, 343880061),
            qm31_from_u32s(1180936792, 1450384302, 1388147362, 343880061),
            qm31_from_u32s(1113827613, 1316166574, 1321038498, 343880061),
            qm31_from_u32s(241305891, 1718819697, 448623205, 343880041),
            qm31_from_u32s(308415070, 1853037425, 515732069, 343880041),
            qm31_from_u32s(375524249, 1987255153, 582840933, 343880041),
            qm31_from_u32s(442633428, 2121472881, 649949797, 343880041),
            qm31_from_u32s(509742607, 108206962, 717058662, 343880041),
            qm31_from_u32s(576851786, 242424690, 784167526, 343880041),
            qm31_from_u32s(643960965, 376642418, 851276390, 343880041),
            qm31_from_u32s(711070144, 510860146, 918385254, 343880041),
            qm31_from_u32s(778179323, 645077874, 985494118, 343880041),
            qm31_from_u32s(845288502, 779295602, 1052602982, 343880041),
            qm31_from_u32s(375831434, 1987255333, 582841113, 343880101),
            qm31_from_u32s(308722255, 1853037605, 515732249, 343880101),
            qm31_from_u32s(510049792, 108207142, 717058842, 343880101),
            qm31_from_u32s(442940613, 2121473061, 649949977, 343880101),
            qm31_from_u32s(644268150, 376642598, 851276570, 343880101),
            qm31_from_u32s(577158971, 242424870, 784167706, 343880101),
            qm31_from_u32s(778486508, 645078054, 985494298, 343880101),
            qm31_from_u32s(711377329, 510860326, 918385434, 343880101),
            qm31_from_u32s(912704866, 913513510, 1119712026, 343880101),
            qm31_from_u32s(845595687, 779295782, 1052603162, 343880101),
            qm31_from_u32s(1046820829, 1181948906, 1253929694, 343880081),
            qm31_from_u32s(1113930008, 1316166634, 1321038558, 343880081),
            qm31_from_u32s(912602471, 913513450, 1119711966, 343880081),
            qm31_from_u32s(979711650, 1047731178, 1186820830, 343880081),
            qm31_from_u32s(778384113, 645077994, 985494238, 343880081),
            qm31_from_u32s(845493292, 779295722, 1052603102, 343880081),
            qm31_from_u32s(644165755, 376642538, 851276510, 343880081),
            qm31_from_u32s(711274934, 510860266, 918385374, 343880081),
            qm31_from_u32s(1583694261, 108207083, 1790800607, 343880081),
            qm31_from_u32s(1650803440, 242424811, 1857909471, 343880081),
            qm31_from_u32s(108388425, 1450385012, 314406248, 343880298),
            qm31_from_u32s(41279246, 1316167284, 247297384, 343880298),
            qm31_from_u32s(2121653714, 1181949555, 180188520, 343880298),
            qm31_from_u32s(2054544535, 1047731827, 113079656, 343880298),
            qm31_from_u32s(1987435356, 913514099, 45970792, 343880298),
            qm31_from_u32s(1920326177, 779296371, 2126345575, 343880297),
            qm31_from_u32s(1853216998, 645078643, 2059236711, 343880297),
            qm31_from_u32s(1786107819, 510860915, 1992127847, 343880297),
            qm31_from_u32s(1718998640, 376643187, 1925018983, 343880297),
            qm31_from_u32s(1651889461, 242425459, 1857910119, 343880297),
            qm31_from_u32s(779367739, 645078582, 985494826, 343880277),
            qm31_from_u32s(846476918, 779296310, 1052603690, 343880277),
            qm31_from_u32s(913586097, 913514038, 1119712554, 343880277),
            qm31_from_u32s(980695276, 1047731766, 1186821418, 343880277),
            qm31_from_u32s(510931023, 108207670, 717059370, 343880277),
            qm31_from_u32s(578040202, 242425398, 784168234, 343880277),
            qm31_from_u32s(645149381, 376643126, 851277098, 343880277),
            qm31_from_u32s(712258560, 510860854, 918385962, 343880277),
            qm31_from_u32s(1316241171, 1718820406, 1522365738, 343880277),
            qm31_from_u32s(1383350350, 1853038134, 1589474602, 343880277),
            qm31_from_u32s(1340598866, 536394231, 1198633759, 502514173),
            qm31_from_u32s(1407708045, 670611959, 1265742623, 502514173),
            qm31_from_u32s(1474817224, 804829687, 1332851487, 502514173),
            qm31_from_u32s(1541926403, 939047415, 1399960351, 502514173),
            qm31_from_u32s(1072162150, 2147006966, 930198302, 502514173),
            qm31_from_u32s(1139271329, 133741047, 997307167, 502514173),
            qm31_from_u32s(1206380508, 267958775, 1064416031, 502514173),
            qm31_from_u32s(1273489687, 402176503, 1131524895, 502514173),
            qm31_from_u32s(1877472298, 1610136055, 1735504671, 502514173),
            qm31_from_u32s(1944581477, 1744353783, 1802613535, 502514173),
            qm31_from_u32s(669619552, 1341700661, 527545181, 502514194),
            qm31_from_u32s(602510373, 1207482933, 460436317, 502514194),
            qm31_from_u32s(535401194, 1073265205, 393327453, 502514194),
            qm31_from_u32s(468292015, 939047477, 326218589, 502514194),
            qm31_from_u32s(401182836, 804829749, 259109725, 502514194),
            qm31_from_u32s(334073657, 670612021, 192000861, 502514194),
            qm31_from_u32s(266964478, 536394293, 124891997, 502514194),
            qm31_from_u32s(199855299, 402176565, 57783133, 502514194),
            qm31_from_u32s(132746120, 267958837, 2138157916, 502514193),
            qm31_from_u32s(65636941, 133741109, 2071049052, 502514193),
            qm31_from_u32s(2146113804, 2147007087, 2003940247, 502514213),
            qm31_from_u32s(65739336, 133741169, 2071049112, 502514213),
            qm31_from_u32s(2011895446, 1878571631, 1869722519, 502514213),
            qm31_from_u32s(2079004625, 2012789359, 1936831383, 502514213),
            qm31_from_u32s(267066873, 536394353, 124892057, 502514214),
            qm31_from_u32s(334176052, 670612081, 192000921, 502514214),
            qm31_from_u32s(132848515, 267958897, 2138157976, 502514213),
            qm31_from_u32s(199957694, 402176625, 57783193, 502514214),
            qm31_from_u32s(1609240372, 1073265263, 1467069335, 502514213),
            qm31_from_u32s(1676349551, 1207482991, 1534178199, 502514213),
            qm31_from_u32s(1475124409, 804829867, 1332851667, 502514233),
            qm31_from_u32s(1408015230, 670612139, 1265742803, 502514233),
            qm31_from_u32s(1609342767, 1073265323, 1467069395, 502514233),
            qm31_from_u32s(1542233588, 939047595, 1399960531, 502514233),
            qm31_from_u32s(1206687693, 267958955, 1064416211, 502514233),
            qm31_from_u32s(1139578514, 133741227, 997307347, 502514233),
            qm31_from_u32s(1340906051, 536394411, 1198633939, 502514233),
            qm31_from_u32s(1273796872, 402176683, 1131525075, 502514233),
            qm31_from_u32s(2011997841, 1878571691, 1869722579, 502514233),
            qm31_from_u32s(1944888662, 1744353963, 1802613715, 502514233),
            qm31_from_u32s(1877062718, 1610135815, 1735504431, 502514093),
            qm31_from_u32s(1944171897, 1744353543, 1802613295, 502514093),
            qm31_from_u32s(2011281076, 1878571271, 1869722159, 502514093),
            qm31_from_u32s(2078390255, 2012788999, 1936831023, 502514093),
            qm31_from_u32s(2145499434, 2147006727, 2003939887, 502514093),
            qm31_from_u32s(65124966, 133740809, 2071048752, 502514093),
            qm31_from_u32s(132234145, 267958537, 2138157616, 502514093),
            qm31_from_u32s(199343324, 402176265, 57782833, 502514094),
            qm31_from_u32s(1340189286, 536393991, 1198633519, 502514093),
            qm31_from_u32s(1407298465, 670611719, 1265742383, 502514093),
            qm31_from_u32s(1206073323, 267958595, 1064415851, 502514113),
            qm31_from_u32s(1138964144, 133740867, 997306987, 502514113),
            qm31_from_u32s(1071854965, 2147006786, 930198122, 502514113),
            qm31_from_u32s(1004745786, 2012789058, 863089258, 502514113),
            qm31_from_u32s(1474510039, 804829507, 1332851307, 502514113),
            qm31_from_u32s(1407400860, 670611779, 1265742443, 502514113),
            qm31_from_u32s(1340291681, 536394051, 1198633579, 502514113),
            qm31_from_u32s(1273182502, 402176323, 1131524715, 502514113),
            qm31_from_u32s(1742946755, 1341700419, 1601286763, 502514113),
            qm31_from_u32s(1675837576, 1207482691, 1534177899, 502514113),
            qm31_from_u32s(535094009, 1073265025, 393327273, 502514134),
            qm31_from_u32s(602203188, 1207482753, 460436137, 502514134),
            qm31_from_u32s(400875651, 804829569, 259109545, 502514134),
            qm31_from_u32s(467984830, 939047297, 326218409, 502514134),
            qm31_from_u32s(266657293, 536394113, 124891817, 502514134),
            qm31_from_u32s(333766472, 670611841, 192000681, 502514134),
            qm31_from_u32s(132438935, 267958657, 2138157736, 502514133),
            qm31_from_u32s(199548114, 402176385, 57782953, 502514134),
            qm31_from_u32s(2145704224, 2147006847, 2003940007, 502514133),
            qm31_from_u32s(65329756, 133740929, 2071048872, 502514133),
            qm31_from_u32s(2011588261, 1878571451, 1869722339, 502514153),
            qm31_from_u32s(1944479082, 1744353723, 1802613475, 502514153),
            qm31_from_u32s(2145806619, 2147006907, 2003940067, 502514153),
            qm31_from_u32s(2078697440, 2012789179, 1936831203, 502514153),
            qm31_from_u32s(132541330, 267958717, 2138157796, 502514153),
            qm31_from_u32s(65432151, 133740989, 2071048932, 502514153),
            qm31_from_u32s(266759688, 536394173, 124891877, 502514154),
            qm31_from_u32s(199650509, 402176445, 57783013, 502514154),
            qm31_from_u32s(1474714829, 804829627, 1332851427, 502514153),
            qm31_from_u32s(1407605650, 670611899, 1265742563, 502514153),
            qm31_from_u32s(266042923, 536393753, 124891457, 502514014),
            qm31_from_u32s(333152102, 670611481, 192000321, 502514014),
            qm31_from_u32s(400261281, 804829209, 259109185, 502514014),
            qm31_from_u32s(467370460, 939046937, 326218049, 502514014),
            qm31_from_u32s(2145089854, 2147006487, 2003939647, 502514013),
            qm31_from_u32s(64715386, 133740569, 2071048512, 502514013),
            qm31_from_u32s(131824565, 267958297, 2138157376, 502514013),
            qm31_from_u32s(198933744, 402176025, 57782593, 502514014),
            qm31_from_u32s(1876653138, 1610135575, 1735504191, 502514013),
            qm31_from_u32s(1943762317, 1744353303, 1802613055, 502514013),
            qm31_from_u32s(1742537175, 1341700179, 1601286523, 502514033),
            qm31_from_u32s(1675427996, 1207482451, 1534177659, 502514033),
            qm31_from_u32s(1608318817, 1073264723, 1467068795, 502514033),
            qm31_from_u32s(1541209638, 939046995, 1399959931, 502514033),
            qm31_from_u32s(1474100459, 804829267, 1332851067, 502514033),
            qm31_from_u32s(1406991280, 670611539, 1265742203, 502514033),
            qm31_from_u32s(1339882101, 536393811, 1198633339, 502514033),
            qm31_from_u32s(1272772922, 402176083, 1131524475, 502514033),
            qm31_from_u32s(131926960, 267958357, 2138157436, 502514033),
            qm31_from_u32s(64817781, 133740629, 2071048572, 502514033),
            qm31_from_u32s(1491955610, 670690004, 1265820668, 502540188),
            qm31_from_u32s(1424846431, 536472276, 1198711804, 502540188),
            qm31_from_u32s(1357737252, 402254548, 1131602940, 502540188),
            qm31_from_u32s(1290628073, 268036820, 1064494076, 502540188),
            qm31_from_u32s(1223518894, 133819092, 997385212, 502540188),
            qm31_from_u32s(1156409715, 2147085011, 930276347, 502540188),
            qm31_from_u32s(1089300536, 2012867283, 863167483, 502540188),
            qm31_from_u32s(1022191357, 1878649555, 796058619, 502540188),
            qm31_from_u32s(955082178, 1744431827, 728949755, 502540188),
            qm31_from_u32s(887972999, 1610214099, 661840891, 502540188),
            qm31_from_u32s(15491601, 2012867234, 1936909257, 502540171),
            qm31_from_u32s(82600780, 2147084962, 2004018121, 502540171),
            qm31_from_u32s(149709959, 133819043, 2071126986, 502540171),
            qm31_from_u32s(216819138, 268036771, 2138235850, 502540171),
            qm31_from_u32s(1894538532, 1475996321, 1668473801, 502540171),
            qm31_from_u32s(1961647711, 1610214049, 1735582665, 502540171),
            qm31_from_u32s(2028756890, 1744431777, 1802691529, 502540171),
            qm31_from_u32s(2095866069, 1878649505, 1869800393, 502540171),
            qm31_from_u32s(552365033, 939125411, 326296523, 502540172),
            qm31_from_u32s(619474212, 1073343139, 393405387, 502540172),
            qm31_from_u32s(149976820, 133819211, 2071127154, 502540227),
            qm31_from_u32s(82867641, 2147085130, 2004018289, 502540227),
            qm31_from_u32s(284195178, 402254667, 57861235, 502540228),
            qm31_from_u32s(217085999, 268036939, 2138236018, 502540227),
            qm31_from_u32s(2029023751, 1744431945, 1802691697, 502540227),
            qm31_from_u32s(1961914572, 1610214217, 1735582833, 502540227),
            qm31_from_u32s(15758462, 2012867402, 1936909425, 502540227),
            qm31_from_u32s(2096132930, 1878649673, 1869800561, 502540227),
            qm31_from_u32s(686850252, 1207561035, 460514419, 502540228),
            qm31_from_u32s(619741073, 1073343307, 393405555, 502540228),
            qm31_from_u32s(820966215, 1475996431, 594732087, 502540208),
            qm31_from_u32s(888075394, 1610214159, 661840951, 502540208),
            qm31_from_u32s(686747857, 1207560975, 460514359, 502540208),
            qm31_from_u32s(753857036, 1341778703, 527623223, 502540208),
            qm31_from_u32s(1089402931, 2012867343, 863167543, 502540208),
            qm31_from_u32s(1156512110, 2147085071, 930276407, 502540208),
            qm31_from_u32s(955184573, 1744431887, 728949815, 502540208),
            qm31_from_u32s(1022293752, 1878649615, 796058679, 502540208),
            qm31_from_u32s(284092783, 402254607, 57861175, 502540208),
            qm31_from_u32s(351201962, 536472335, 124970039, 502540208),
            qm31_from_u32s(2028449705, 1744431597, 1802691349, 502540111),
            qm31_from_u32s(1961340526, 1610213869, 1735582485, 502540111),
            qm31_from_u32s(1894231347, 1475996141, 1668473621, 502540111),
            qm31_from_u32s(1827122168, 1341778413, 1601364757, 502540111),
            qm31_from_u32s(149402774, 133818863, 2071126806, 502540111),
            qm31_from_u32s(82293595, 2147084782, 2004017941, 502540111),
            qm31_from_u32s(15184416, 2012867054, 1936909077, 502540111),
            qm31_from_u32s(2095558884, 1878649325, 1869800213, 502540111),
            qm31_from_u32s(417839490, 670689775, 192078615, 502540112),
            qm31_from_u32s(350730311, 536472047, 124969751, 502540112),
            qm31_from_u32s(551955453, 939125171, 326296283, 502540092),
            qm31_from_u32s(619064632, 1073342899, 393405147, 502540092),
            qm31_from_u32s(686173811, 1207560627, 460514011, 502540092),
            qm31_from_u32s(753282990, 1341778355, 527622875, 502540092),
            qm31_from_u32s(820392169, 1475996083, 594731739, 502540092),
            qm31_from_u32s(887501348, 1610213811, 661840603, 502540092),
            qm31_from_u32s(954610527, 1744431539, 728949467, 502540092),
            qm31_from_u32s(1021719706, 1878649267, 796058331, 502540092),
            qm31_from_u32s(15082021, 2012866994, 1936909017, 502540091),
            qm31_from_u32s(82191200, 2147084722, 2004017881, 502540091),
            qm31_from_u32s(686480996, 1207560807, 460514191, 502540152),
            qm31_from_u32s(619371817, 1073343079, 393405327, 502540152),
            qm31_from_u32s(820699354, 1475996263, 594731919, 502540152),
            qm31_from_u32s(753590175, 1341778535, 527623055, 502540152),
            qm31_from_u32s(954917712, 1744431719, 728949647, 502540152),
            qm31_from_u32s(887808533, 1610213991, 661840783, 502540152),
            qm31_from_u32s(1089136070, 2012867175, 863167375, 502540152),
            qm31_from_u32s(1022026891, 1878649447, 796058511, 502540152),
            qm31_from_u32s(149607564, 133818983, 2071126926, 502540151),
            qm31_from_u32s(82498385, 2147084902, 2004018061, 502540151),
            qm31_from_u32s(1357470391, 402254380, 1131602772, 502540132),
            qm31_from_u32s(1424579570, 536472108, 1198711636, 502540132),
            qm31_from_u32s(1223252033, 133818924, 997385044, 502540132),
            qm31_from_u32s(1290361212, 268036652, 1064493908, 502540132),
            qm31_from_u32s(1089033675, 2012867115, 863167315, 502540132),
            qm31_from_u32s(1156142854, 2147084843, 930276179, 502540132),
            qm31_from_u32s(954815317, 1744431659, 728949587, 502540132),
            qm31_from_u32s(1021924496, 1878649387, 796058451, 502540132),
            qm31_from_u32s(820596959, 1475996203, 594731859, 502540132),
            qm31_from_u32s(887706138, 1610213931, 661840723, 502540132),
            qm31_from_u32s(417429910, 670689535, 192078375, 502540032),
            qm31_from_u32s(350320731, 536471807, 124969511, 502540032),
            qm31_from_u32s(283211552, 402254079, 57860647, 502540032),
            qm31_from_u32s(216102373, 268036351, 2138235430, 502540031),
            qm31_from_u32s(148993194, 133818623, 2071126566, 502540031),
            qm31_from_u32s(81884015, 2147084542, 2004017701, 502540031),
            qm31_from_u32s(14774836, 2012866814, 1936908837, 502540031),
            qm31_from_u32s(2095149304, 1878649085, 1869799973, 502540031),
            qm31_from_u32s(954303342, 1744431359, 728949287, 502540032),
            qm31_from_u32s(887194163, 1610213631, 661840423, 502540032),
            qm31_from_u32s(1088419305, 2012866755, 863166955, 502540012),
            qm31_from_u32s(1155528484, 2147084483, 930275819, 502540012),
            qm31_from_u32s(1222637663, 133818564, 997384684, 502540012),
            qm31_from_u32s(1289746842, 268036292, 1064493548, 502540012),
            qm31_from_u32s(819982589, 1475995843, 594731499, 502540012),
            qm31_from_u32s(887091768, 1610213571, 661840363, 502540012),
            qm31_from_u32s(954200947, 1744431299, 728949227, 502540012),
            qm31_from_u32s(1021310126, 1878649027, 796058091, 502540012),
            qm31_from_u32s(551545873, 939124931, 326296043, 502540012),
            qm31_from_u32s(618655052, 1073342659, 393404907, 502540012),
            qm31_from_u32s(732050662, 1341756416, 527600936, 502532779),
            qm31_from_u32s(799159841, 1475974144, 594709800, 502532779),
            qm31_from_u32s(597832304, 1073320960, 393383208, 502532779),
            qm31_from_u32s(664941483, 1207538688, 460492072, 502532779),
            qm31_from_u32s(463613946, 804885504, 259165480, 502532779),
            qm31_from_u32s(530723125, 939103232, 326274344, 502532779),
            qm31_from_u32s(329395588, 536450048, 124947752, 502532779),
            qm31_from_u32s(396504767, 670667776, 192056616, 502532779),
            qm31_from_u32s(1268924094, 268014593, 1064471849, 502532779),
            qm31_from_u32s(1336033273, 402232321, 1131580713, 502532779),
            qm31_from_u32s(61061267, 2147062843, 2003996002, 502532798),
            qm31_from_u32s(2141435735, 2012845114, 1936887138, 502532798),
            qm31_from_u32s(195279625, 268014652, 2138213731, 502532798),
            qm31_from_u32s(128170446, 133796924, 2071104867, 502532798),
            qm31_from_u32s(329497983, 536450108, 124947812, 502532799),
            qm31_from_u32s(262388804, 402232380, 57838948, 502532799),
            qm31_from_u32s(463716341, 804885564, 259165540, 502532799),
            qm31_from_u32s(396607162, 670667836, 192056676, 502532799),
            qm31_from_u32s(597934699, 1073321020, 393383268, 502532799),
            qm31_from_u32s(530825520, 939103292, 326274404, 502532799),
            qm31_from_u32s(2074019371, 1878627206, 1869778094, 502532738),
            qm31_from_u32s(2141128550, 2012844934, 1936886958, 502532738),
            qm31_from_u32s(60754082, 2147062663, 2003995822, 502532738),
            qm31_from_u32s(127863261, 133796744, 2071104687, 502532738),
            qm31_from_u32s(194972440, 268014472, 2138213551, 502532738),
            qm31_from_u32s(262081619, 402232200, 57838768, 502532739),
            qm31_from_u32s(329190798, 536449928, 124947632, 502532739),
            qm31_from_u32s(396299977, 670667656, 192056496, 502532739),
            qm31_from_u32s(463409156, 804885384, 259165360, 502532739),
            qm31_from_u32s(530518335, 939103112, 326274224, 502532739),
            qm31_from_u32s(1403040057, 536449989, 1198689517, 502532759),
            qm31_from_u32s(1335930878, 402232261, 1131580653, 502532759),
            qm31_from_u32s(1268821699, 268014533, 1064471789, 502532759),
            qm31_from_u32s(1201712520, 133796805, 997362925, 502532759),
            qm31_from_u32s(1671476773, 1073320901, 1467124973, 502532759),
            qm31_from_u32s(1604367594, 939103173, 1400016109, 502532759),
            qm31_from_u32s(1537258415, 804885445, 1332907245, 502532759),
            qm31_from_u32s(1470149236, 670667717, 1265798381, 502532759),
            qm31_from_u32s(866166625, 1610191812, 661818604, 502532759),
            qm31_from_u32s(799057446, 1475974084, 594709740, 502532759),
            qm31_from_u32s(195546486, 268014820, 2138213899, 502532854),
            qm31_from_u32s(262655665, 402232548, 57839116, 502532855),
            qm31_from_u32s(61328128, 2147063011, 2003996170, 502532854),
            qm31_from_u32s(128437307, 133797092, 2071105035, 502532854),
            qm31_from_u32s(463983202, 804885732, 259165708, 502532855),
            qm31_from_u32s(531092381, 939103460, 326274572, 502532855),
            qm31_from_u32s(329764844, 536450276, 124947980, 502532855),
            qm31_from_u32s(396874023, 670668004, 192056844, 502532855),
            qm31_from_u32s(732419918, 1341756644, 527601164, 502532855),
            qm31_from_u32s(799529097, 1475974372, 594710028, 502532855),
            qm31_from_u32s(1672050819, 1073321249, 1467125321, 502532875),
            qm31_from_u32s(1604941640, 939103521, 1400016457, 502532875),
            qm31_from_u32s(1806269177, 1341756705, 1601343049, 502532875),
            qm31_from_u32s(1739159998, 1207538977, 1534234185, 502532875),
            qm31_from_u32s(1403614103, 536450337, 1198689865, 502532875),
            qm31_from_u32s(1336504924, 402232609, 1131581001, 502532875),
            qm31_from_u32s(1537832461, 804885793, 1332907593, 502532875),
            qm31_from_u32s(1470723282, 670668065, 1265798729, 502532875),
            qm31_from_u32s(1135177387, 2147063072, 930254408, 502532875),
            qm31_from_u32s(1068068208, 2012845344, 863145544, 502532875),
            qm31_from_u32s(1537525276, 804885613, 1332907413, 502532815),
            qm31_from_u32s(1604634455, 939103341, 1400016277, 502532815),
            qm31_from_u32s(1671743634, 1073321069, 1467125141, 502532815),
            qm31_from_u32s(1738852813, 1207538797, 1534234005, 502532815),
            qm31_from_u32s(1269088560, 268014701, 1064471957, 502532815),
            qm31_from_u32s(1336197739, 402232429, 1131580821, 502532815),
            qm31_from_u32s(1403306918, 536450157, 1198689685, 502532815),
            qm31_from_u32s(1470416097, 670667885, 1265798549, 502532815),
            qm31_from_u32s(1000651844, 1878627436, 796036500, 502532815),
            qm31_from_u32s(1067761023, 2012845164, 863145364, 502532815),
            qm31_from_u32s(866535881, 1610192040, 661818832, 502532835),
            qm31_from_u32s(799426702, 1475974312, 594709968, 502532835),
            qm31_from_u32s(732317523, 1341756584, 527601104, 502532835),
            qm31_from_u32s(665208344, 1207538856, 460492240, 502532835),
            qm31_from_u32s(598099165, 1073321128, 393383376, 502532835),
            qm31_from_u32s(530989986, 939103400, 326274512, 502532835),
            qm31_from_u32s(463880807, 804885672, 259165648, 502532835),
            qm31_from_u32s(396771628, 670667944, 192056784, 502532835),
            qm31_from_u32s(1403409313, 536450217, 1198689745, 502532835),
            qm31_from_u32s(1336300134, 402232489, 1131580881, 502532835),
            qm31_from_u32s(1806576362, 1341756885, 1601343229, 502532935),
            qm31_from_u32s(1873685541, 1475974613, 1668452093, 502532935),
            qm31_from_u32s(1672358004, 1073321429, 1467125501, 502532935),
            qm31_from_u32s(1739467183, 1207539157, 1534234365, 502532935),
            qm31_from_u32s(1538139646, 804885973, 1332907773, 502532935),
            qm31_from_u32s(1605248825, 939103701, 1400016637, 502532935),
            qm31_from_u32s(1403921288, 536450517, 1198690045, 502532935),
            qm31_from_u32s(1471030467, 670668245, 1265798909, 502532935),
            qm31_from_u32s(1269702930, 268015061, 1064472317, 502532935),
            qm31_from_u32s(1336812109, 402232789, 1131581181, 502532935),
            qm31_from_u32s(1135586967, 2147063312, 930254648, 502532955),
            qm31_from_u32s(1068477788, 2012845584, 863145784, 502532955),
            qm31_from_u32s(1269805325, 268015121, 1064472377, 502532955),
        ];
        let interaction_columns = [
            qm31_from_u32s(1005168032, 79980996, 1847888101, 1941984119),
            qm31_from_u32s(1072277211, 214198724, 1914996965, 1941984119),
            qm31_from_u32s(1139386390, 348416452, 1982105829, 1941984119),
            qm31_from_u32s(1206495569, 482634180, 2049214693, 1941984119),
            qm31_from_u32s(736731316, 1690593731, 1579452644, 1941984119),
            qm31_from_u32s(803840495, 1824811459, 1646561508, 1941984119),
            qm31_from_u32s(870949674, 1959029187, 1713670372, 1941984119),
            qm31_from_u32s(938058853, 2093246915, 1780779236, 1941984119),
            qm31_from_u32s(1542041464, 1153722820, 237275366, 1941984120),
            qm31_from_u32s(1609150643, 1287940548, 304384230, 1941984120),
        ];
        let component_data = TestComponentData::from_values(
            &mut context,
            &trace_columns,
            &interaction_columns,
            qm31_from_u32s(1115374022, 1127856551, 489657863, 643630026),
            32768,
        );
        let random_coeff =
            context.new_var(qm31_from_u32s(474642921, 876336632, 1911695779, 974600512));
        let interaction_elements = [
            context.new_var(qm31_from_u32s(445623802, 202571636, 1360224996, 131355117)),
            context.new_var(qm31_from_u32s(476823935, 939223384, 62486082, 122423602)),
        ];
        let preprocessed_columns = HashMap::from([(
            PreProcessedColumnId { id: "seq_15".to_owned() },
            context.constant(qm31_from_u32s(735272696, 1215403647, 795393303, 879304430)),
        )]);
        let public_params = HashMap::from([]);
        let mut accumulator = CompositionConstraintAccumulator::new(
            &mut context,
            preprocessed_columns,
            public_params,
            random_coeff,
            interaction_elements,
        );
        component.evaluate(&mut context, &component_data, &mut accumulator);
        let claimed_sum =
            context.new_var(qm31_from_u32s(1398335417, 314974026, 1722107152, 821933968));
        accumulator.finalize_logup_in_pairs(
            &mut context,
            <TestComponentData as ComponentDataTrait<QM31>>::interaction_columns(&component_data),
            &component_data,
            claimed_sum,
        );

        let result = accumulator.finalize();
        let result_value = context.get(result);
        assert_eq!(result_value, KECCAK_AGGREGATOR_SAMPLE_EVAL_RESULT)
    }
}
