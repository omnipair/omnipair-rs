use super::*;

    #[test]
    fn virtual_reserves_match_spot_when_pessimistic_price_matches_spot() {
        let x = 1_000 * NAD as u128;
        let y = 500 * NAD as u128;

        let (x_virt, y_virt) =
            construct_normalized_virtual_reserves_at_pessimistic_price(x, y, NAD / 2, NAD / 2)
                .unwrap();

        assert_eq!(x_virt, x);
        assert_eq!(y_virt, y);
    }

    #[test]
    fn virtual_reserves_move_to_lower_pessimistic_price_coordinate() {
        let x = 1_000 * NAD as u128;
        let y = 1_000 * NAD as u128;

        let (x_virt, y_virt) =
            construct_normalized_virtual_reserves_at_pessimistic_price(x, y, 2 * NAD, NAD / 4)
                .unwrap();

        assert_eq!(x_virt, 2_000 * NAD as u128);
        assert_eq!(y_virt, 500 * NAD as u128);
        assert_eq!(
            x_virt.checked_mul(y_virt).unwrap(),
            x.checked_mul(y).unwrap()
        );
    }

    #[test]
    fn raw_amount_out_matches_constant_product_rounding_down() {
        let dy = calculate_raw_amount_out(1_000, 2_000, 100).unwrap();

        assert_eq!(dy, 181);
    }
