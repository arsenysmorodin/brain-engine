# Запрос исходных данных фармакокинетики

Черновик для авторов; сообщения не отправлены. Обновлено 25.10.2026.

Нужны He et al. 2017 (10.1089/jcr.2017.0006), Mondal et al. 2022 (10.53520/jen2022.103124), Wang et al. 2020 (10.1016/j.jchromb.2020.122278). Контакт следует брать из актуальной страницы статьи, не угадывать. Для Wang также нужен полный текст/рукопись с подписями рисунков и дополнительными материалами.

## Draft in English

Subject: Request for de-identified concentration-time PK data and clarification of reported units

Dear authors,

We are developing an open-source, educational Rust simulation of oral caffeine, methylliberine and theacrine pharmacokinetics with a separate illustrative brain circulation model. We would like to calibrate plasma concentration-time curves to published measurements, without treating illustrative brain parameters as experimentally measured.

Could you share the de-identified individual concentration-time data for the single-compound and combination conditions in the studies cited above, or direct us to an accessible repository/supplement? A CSV with the following fields would be sufficient:

- Anonymous participant ID, study/cohort/period/sequence and treatment condition.
- Analyte, dose in mg, formulation, actual administration and sampling times, and concentration with units.
- Predose/background values, BLQ flags, LLOQ/ULOQ, missing-sample flags and relevant dilution corrections.
- Sampling through the complete terminal observation period, including any 48-hour samples.
- Available non-identifying covariates needed to interpret dose/exposure (e.g. body mass), with your data-sharing restrictions.

We would also appreciate clarification of the following:

1. Are plotted concentrations absolute, dose-normalized, or scaled by a dilution/other factor? In particular, He 2017 has dose-normalization statements associated with Cmax/AUC, while absolute caffeine/theacrine scales in the later publications differ substantially. We are preserving each printed axis and do not interpret the cross-study difference as a causal interaction.
2. For Wang 2020 Figures 3–5, do markers represent arithmetic/geometric means or individual values, and are error bars SD, SEM or another interval? The abstract describes 24-hour collection, while Figure 5 includes 48 hours. What were the actual nominal and individual sample times and full dosing conditions?
3. For He 2017, can the individual theacrine-only (especially 125 mg) concentration-time data be supplied? Early overlapping markers cannot be reliably digitized into a complete mean curve.
4. How were predose concentrations, BLQ observations and terminal slopes handled? Are the reported summary PK endpoints arithmetic/geometric means, medians or other estimates?
5. Under what terms may the de-identified data, derived digitized points and fitted parameters be redistributed with attribution? Is participant-level redistribution restricted to a data-use agreement?

We can share our digitization audit (source image hashes, pixel coordinates, axis conversion and exclusions), model equations and reproducible fitting report. We will label any graph-derived fit as preliminary and will not describe fitting to the same data as independent validation.

Thank you for considering this request.
