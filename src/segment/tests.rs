use super::*;

#[test]
fn test_enc_no_flags() {
    test_enc_flags_helper(false, false, false, 0b0000_0000)
}

#[test]
fn test_enc_all_flags() {
    test_enc_flags_helper(true, true, true, 0b1110_0000)
}

#[test]
fn test_enc_syn_flag() {
    test_enc_flags_helper(true, false, false, 0b1000_0000)
}

#[test]
fn test_enc_ack_flag() {
    test_enc_flags_helper(false, true, false, 0b0100_0000)
}

#[test]
fn test_enc_fin_flag() {
    test_enc_flags_helper(false, false, true, 0b0010_0000)
}

fn test_enc_flags_helper(syn: bool, ack: bool, fin: bool, exp_byte: u8) {
    // Arrange
    let data = vec![7, 8, 9];
    let seg = Segment::new(syn, ack, fin, 123, 258, &data);

    // Act
    let enc = seg.encode();

    // Assert
    let exp_len = 9 + data.len();
    assert_eq!(enc.len(), exp_len);
    assert_eq!(enc[0], exp_byte);
    assert_eq!(&enc[1..5], vec![0, 0, 0, 123]);
    assert_eq!(&enc[5..9], vec![0, 0, 1, 2]);
    assert_eq!(&enc[9..exp_len], &data);
}

#[test]
fn test_enc_seq_num_0() {
    test_enc_seq_num_helper(0, &[0,0,0,0]);
}

#[test]
fn test_enc_seq_num_1() {
    test_enc_seq_num_helper(1, &[0,0,0,1]);
}

#[test]
fn test_enc_seq_num_255() {
    test_enc_seq_num_helper(255, &[0,0,0,255]);
}

#[test]
fn test_enc_seq_num_256() {
    test_enc_seq_num_helper(256, &[0,0,1,0]);
}

#[test]
fn test_enc_seq_num_257() {
    test_enc_seq_num_helper(257, &[0,0,1,1]);
}

#[test]
fn test_enc_seq_num_max() {
    test_enc_seq_num_helper(4294967295, &[255,255,255,255]);
}

fn test_enc_seq_num_helper(seq_num: u32, exp_enc: &[u8]) {
    // Arrange
    let data = vec![7, 8, 9];
    let seg = Segment::new(false, true, false, seq_num, 258, &data);

    // Act
    let enc = seg.encode();

    // Assert
    let exp_len = 9 + data.len();
    assert_eq!(enc.len(), exp_len);
    assert_eq!(enc[0], 0b0100_0000);
    assert_eq!(&enc[1..5], exp_enc);
    assert_eq!(&enc[5..9], vec![0, 0, 1, 2]);
    assert_eq!(&enc[9..exp_len], &data);
}
