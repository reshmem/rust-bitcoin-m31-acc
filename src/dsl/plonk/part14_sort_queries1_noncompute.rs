use crate::dsl::plonk::hints::{Hints, LOG_N_ROWS};
use crate::dsl::tools::{unzip_elements, Zipper};
use crate::dsl::{load_data_types, load_functions};
use anyhow::Result;
use bitcoin_script_dsl::dsl::{Element, DSL};
use bitcoin_script_dsl::options::Options;
use itertools::Itertools;
use std::collections::HashMap;

pub fn generate_dsl(hints: &Hints, cache: &mut HashMap<String, Zipper>) -> Result<DSL> {
    let mut dsl = DSL::new();

    load_data_types(&mut dsl)?;
    load_functions(&mut dsl)?;

    // assume, that the inputs, contain the hashes from the previous step
    let shared_information_hash = dsl.alloc_input(
        "hash",
        Element::Str(
            cache
                .get("shared_information")
                .unwrap()
                .hash
                .as_ref()
                .to_vec(),
        ),
    )?;

    let sort_queries1_hash = dsl.alloc_input(
        "hash",
        Element::Str(
            cache
                .get("sort_queries1")
                .unwrap()
                .hash
                .as_ref()
                .to_vec()
                .to_vec(),
        ),
    )?;

    // unpack `sort_queries1_hash`
    let res = unzip_elements(
        &mut dsl,
        sort_queries1_hash,
        cache.get("sort_queries1").unwrap(),
    )?;
    assert_eq!(res.len(), (8 + LOG_N_ROWS + 5) as usize);

    let mut res = res.into_iter();

    let mut queries = vec![];
    for _ in 0..8 {
        queries.push(res.next().unwrap());
    }

    let mut fri_tree_commitments_vars = vec![];
    for _ in 0..LOG_N_ROWS {
        fri_tree_commitments_vars.push(res.next().unwrap());
    }

    let trace_queried_results_hash = res.next().unwrap();
    let interaction_queried_results_hash = res.next().unwrap();
    let constant_queried_results_hash = res.next().unwrap();
    let composition_queried_results_hash = res.next().unwrap();
    let twiddles_hash = res.next().unwrap();

    assert!(res.next().is_none());

    /*let mut folding_intermediate_results_vars = vec![];
    for (&query, fold_hints) in queries
        .iter()
        .zip(hints.per_query_fold_hints.iter())
        .take(3)
    {
        let queries = dsl.execute("decompose_positions_to_5", &[query])?;
        let mut tmp = vec![];
        for ((&commitment, proof), &query) in fri_tree_commitments_vars
            .iter()
            .zip(fold_hints.twin_proofs.iter())
            .zip(queries.iter())
        {
            let res = dsl.execute_with_options(
                "merkle_twin_tree_4",
                &[commitment, query],
                &Options::new()
                    .with_multi_u32(
                        "left",
                        vec![
                            proof.left[0].0,
                            proof.left[1].0,
                            proof.left[2].0,
                            proof.left[3].0,
                        ],
                    )
                    .with_multi_u32(
                        "right",
                        vec![
                            proof.right[0].0,
                            proof.right[1].0,
                            proof.right[2].0,
                            proof.right[3].0,
                        ],
                    )
                    .with_multi_binary(
                        "path",
                        proof
                            .path
                            .siblings
                            .iter()
                            .map(|x| x.as_ref().to_vec())
                            .collect_vec(),
                    ),
            )?;

            let left_first = dsl.execute("cm31_from_real_and_imag", &[res[0], res[1]])?[0];
            let left_second = dsl.execute("cm31_from_real_and_imag", &[res[2], res[3]])?[0];
            let left = dsl.execute("qm31_from_first_and_second", &[left_first, left_second])?[0];

            let right_first = dsl.execute("cm31_from_real_and_imag", &[res[4], res[5]])?[0];
            let right_second = dsl.execute("cm31_from_real_and_imag", &[res[6], res[7]])?[0];
            let right = dsl.execute("qm31_from_first_and_second", &[right_first, right_second])?[0];

            tmp.push((left, right));
        }
        folding_intermediate_results_vars.push(tmp);
    }*/

    Ok(dsl)
}
