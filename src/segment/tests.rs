use super::*;

#[test]
fn test_enc_no_flags() {
    // Arrange
    let data = vec![7, 8, 9];
    let seg = Segment::new(false, false, false, 123, 258, &data);

    // Act
    let enc = seg.encode();

    // Assert
    let len = 9 + data.len();
    assert_eq!(enc.len(), len);
    assert_eq!(enc[0], 0);
    assert_eq!(&enc[1..5], vec![0, 0, 0, 123]);
    assert_eq!(&enc[5..9], vec![0, 0, 1, 2]);
    assert_eq!(&enc[9..len], &data);
}

#[test]
fn test_enc_all_flags() {
    // Arrange
    let data = vec![7, 8, 9];
    let seg = Segment::new(true, true, true, 123, 258, &data);

    // Act
    let enc = seg.encode();

    // Assert
    let len = 9 + data.len();
    assert_eq!(enc.len(), len);
    assert_eq!(enc[0], 7);
    assert_eq!(&enc[1..5], vec![0, 0, 0, 123]);
    assert_eq!(&enc[5..9], vec![0, 0, 1, 2]);
    assert_eq!(&enc[9..len], &data);
}