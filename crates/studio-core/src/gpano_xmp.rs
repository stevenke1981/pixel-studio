use crate::{PhotoError, Result};

const XMP_NAMESPACE: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

pub fn build_gpano_xmp(width: u32, height: u32) -> String {
    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description
      xmlns:GPano="http://ns.google.com/photos/1.0/panorama/"
      GPano:UsePanoramaViewer="True"
      GPano:ProjectionType="equirectangular"
      GPano:CroppedAreaImageWidthPixels="{width}"
      GPano:CroppedAreaImageHeightPixels="{height}"
      GPano:FullPanoWidthPixels="{width}"
      GPano:FullPanoHeightPixels="{height}"
      GPano:CroppedAreaLeftPixels="0"
      GPano:CroppedAreaTopPixels="0"/>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#
    )
}

pub fn inject_xmp_into_jpeg(jpeg: &[u8], xmp: &str) -> Result<Vec<u8>> {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return Err(PhotoError::InvalidJpeg);
    }

    let mut payload = Vec::with_capacity(XMP_NAMESPACE.len() + xmp.len());
    payload.extend_from_slice(XMP_NAMESPACE);
    payload.extend_from_slice(xmp.as_bytes());

    let segment_len = payload.len() + 2;
    if segment_len > u16::MAX as usize {
        return Err(PhotoError::XmpTooLarge);
    }

    let mut output = Vec::with_capacity(jpeg.len() + payload.len() + 4);
    output.extend_from_slice(&jpeg[0..2]);
    output.extend_from_slice(&[0xFF, 0xE1]);
    output.extend_from_slice(&(segment_len as u16).to_be_bytes());
    output.extend_from_slice(&payload);
    output.extend_from_slice(&jpeg[2..]);
    Ok(output)
}

pub fn contains_gpano_marker(bytes: &[u8]) -> bool {
    bytes
        .windows(b"GPano:UsePanoramaViewer".len())
        .any(|window| window == b"GPano:UsePanoramaViewer")
}

#[cfg(test)]
mod tests {
    use super::{build_gpano_xmp, contains_gpano_marker, inject_xmp_into_jpeg};

    #[test]
    fn builds_gpano_metadata() {
        let xmp = build_gpano_xmp(100, 50);
        assert!(xmp.contains("GPano:UsePanoramaViewer"));
        assert!(xmp.contains("FullPanoWidthPixels=\"100\""));
    }

    #[test]
    fn injects_xmp_after_soi() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xD9];
        let injected = inject_xmp_into_jpeg(&jpeg, &build_gpano_xmp(100, 50)).unwrap();
        assert_eq!(&injected[0..4], &[0xFF, 0xD8, 0xFF, 0xE1]);
        assert!(contains_gpano_marker(&injected));
    }

    #[test]
    fn rejects_non_jpeg_bytes() {
        let error = inject_xmp_into_jpeg(b"not jpeg", "xmp").unwrap_err();
        assert!(format!("{error}").contains("invalid JPEG"));
    }
}
