use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::pcep::{
    PCEP_MSG_OPEN, PCEP_MSG_PCREP, PCEP_MSG_PCREQ, PCEP_PORT, PcepMessage, PcepObject, PcepSession,
};

#[test]
fn test_pcep_open_and_sr_path_computation() {
    let open = PcepMessage::build_open(30, 120, 1);
    let raw_open = open.serialize();

    let parsed_open = PcepMessage::parse(&raw_open).unwrap();
    assert_eq!(parsed_open.header.msg_type, PCEP_MSG_OPEN);
    if let PcepObject::Open {
        version,
        keepalive_s,
        deadtimer_s,
        sid,
    } = parsed_open.objects[0]
    {
        assert_eq!(version, 1);
        assert_eq!(keepalive_s, 30);
        assert_eq!(deadtimer_s, 120);
        assert_eq!(sid, 1);
    } else {
        panic!("Expected Open object");
    }

    let src = Ipv4Address::new(10, 0, 0, 1);
    let dst = Ipv4Address::new(10, 0, 0, 4);
    let req = PcepMessage::build_pcreq(201, src, dst);
    let raw_req = req.serialize();

    let parsed_req = PcepMessage::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.header.msg_type, PCEP_MSG_PCREQ);

    let mut session = PcepSession::new();
    let rep = session.compute_path(&parsed_req).unwrap();
    let raw_rep = rep.serialize();

    let parsed_rep = PcepMessage::parse(&raw_rep).unwrap();
    assert_eq!(parsed_rep.header.msg_type, PCEP_MSG_PCREP);

    if let PcepObject::SrEro { sids } = &parsed_rep.objects[1] {
        assert_eq!(sids.len(), 3);
        assert_eq!(sids[0], 16001);
        assert_eq!(sids[1], 24001);
        assert_eq!(sids[2], 16004);
    } else {
        panic!("Expected SR-ERO object");
    }

    assert_eq!(PCEP_PORT, 4189);
}
