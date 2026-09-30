use super::*;
use crate::work::NoopWorkControl;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual {actual}, expected {expected}"
    );
}

// Numeric oracles from the actual elkjs 0.9.3 bundle / ELK 0.9.1.
// Inputs use elk.algorithm=radial, omitNodeMicroLayout=true, node and edge order below.
// Default node positions are zero. The container case overrides only spacing and padding.
#[test]
fn matches_elkjs_default_wedges_overlap_extension_routing_and_disconnected_semantics() {
    {
        // single
        let nodes = vec![Node {
            width: 40.0,
            height: 30.0,
            ..Node::default()
        }];
        let edges = vec![];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 64.0);
        close(result.height, 54.0);
        let expected = [(12.0, 12.0)];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
    }
    {
        // star
        let nodes = vec![
            Node {
                width: 40.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 60.0,
                height: 20.0,
                ..Node::default()
            },
            Node {
                width: 20.0,
                height: 50.0,
                ..Node::default()
            },
            Node {
                width: 30.0,
                height: 30.0,
                ..Node::default()
            },
        ];
        let edges = vec![Edge::new(0, 1), Edge::new(0, 2), Edge::new(0, 3)];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 149.42794885482215);
        close(result.height, 155.83548211372658);
        let expected = [
            (60.003719314851395, 58.90612697654616),
            (70.21441927407362, 123.8354821137266),
            (12.0, 23.695642140987488),
            (107.42794885482215, 12.0),
        ];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        let edge = result.edges[0].unwrap();
        close(edge.start.x, 85.06235040965274);
        close(edge.start.y, 88.90612697654616);
        close(edge.end.x, 96.84199854420606);
        close(edge.end.y, 123.83548211372658);
        let edge = result.edges[1].unwrap();
        close(edge.start.x, 60.003719314851395);
        close(edge.start.y, 65.2134137735486);
        close(edge.end.x, 32.0);
        close(edge.end.y, 53.041998742486264);
        let edge = result.edges[2].unwrap();
        close(edge.start.x, 93.57046387719296);
        close(edge.start.y, 58.90612697654616);
        close(edge.end.x, 108.86120429248056);
        close(edge.end.y, 42.0);
    }
    {
        // deep
        let nodes = vec![
            Node {
                width: 40.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 60.0,
                height: 20.0,
                ..Node::default()
            },
            Node {
                width: 20.0,
                height: 50.0,
                ..Node::default()
            },
            Node {
                width: 90.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 20.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 50.0,
                height: 40.0,
                ..Node::default()
            },
        ];
        let edges = vec![
            Edge::new(0, 1),
            Edge::new(0, 2),
            Edge::new(1, 3),
            Edge::new(1, 4),
            Edge::new(2, 5),
        ];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 361.2234690410328);
        close(result.height, 378.58019607607);
        let expected = [
            (176.65696297921934, 157.45207912729987),
            (102.8737099483126, 232.6781186909498),
            (250.44021601012608, 77.22603956364993),
            (214.21188059876152, 336.58019607607),
            (12.0, 58.32418098580662),
            (299.2234690410328, 12.0),
        ];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        let edge = result.edges[0].unwrap();
        close(edge.start.x, 183.03311630627056);
        close(edge.start.y, 187.45207912729987);
        close(edge.end.x, 141.9562743969451);
        close(edge.end.y, 232.6781186909498);
        let edge = result.edges[1].unwrap();
        close(edge.start.x, 210.2808096521681);
        close(edge.start.y, 157.45207912729987);
        close(edge.end.x, 250.44021601012605);
        close(edge.end.y, 113.23614585747933);
        let edge = result.edges[2].unwrap();
        close(edge.start.x, 144.47478989866724);
        close(edge.start.y, 252.6781186909498);
        close(edge.end.x, 241.81026067322955);
        close(edge.end.y, 336.58019607607);
        let edge = result.edges[3].unwrap();
        close(edge.start.x, 125.19302511830934);
        close(edge.start.y, 232.67811869094976);
        close(edge.end.x, 32.0);
        close(edge.end.y, 111.34385334749076);
        let edge = result.edges[4].unwrap();
        close(edge.start.x, 270.44021601012605);
        close(edge.start.y, 91.21593326982054);
        close(edge.end.x, 306.05834014376774);
        close(edge.end.y, 52.0);
    }
    {
        // forest
        let nodes = vec![
            Node {
                width: 40.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 60.0,
                height: 20.0,
                ..Node::default()
            },
            Node {
                width: 20.0,
                height: 50.0,
                ..Node::default()
            },
        ];
        let edges = vec![Edge::new(0, 1)];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 137.2455532033676);
        close(result.height, 89.0);
        let expected = [
            (85.24555320336759, 12.0),
            (12.0, 17.000000000000007),
            (105.24555320336759, 27.0),
        ];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        let edge = result.edges[0].unwrap();
        close(edge.start.x, 85.24555320336759);
        close(edge.start.y, 27.000000000000004);
        close(edge.end.x, 72.0);
        close(edge.end.y, 27.000000000000004);
    }
    {
        // dag
        let nodes = vec![
            Node {
                width: 40.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 60.0,
                height: 20.0,
                ..Node::default()
            },
            Node {
                width: 20.0,
                height: 50.0,
                ..Node::default()
            },
            Node {
                width: 40.0,
                height: 40.0,
                ..Node::default()
            },
        ];
        let edges = vec![
            Edge::new(0, 1),
            Edge::new(0, 2),
            Edge::new(1, 3),
            Edge::new(2, 3),
        ];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 90.58786497178826);
        close(result.height, 243.0101656940116);
        let expected = [
            (27.529288323929464, 143.00677712934106),
            (12.0, 211.0101656940116),
            (43.058576647858864, 70.00338856467053),
            (38.587864971788264, 12.0),
        ];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        let edge = result.edges[0].unwrap();
        close(edge.start.x, 46.212861910232554);
        close(edge.start.y, 173.00677712934106);
        close(edge.end.x, 42.87761760913127);
        close(edge.end.y, 211.0101656940116);
        let edge = result.edges[1].unwrap();
        close(edge.start.x, 48.84571473762635);
        close(edge.start.y, 143.00677712934106);
        close(edge.end.x, 50.864532625030705);
        close(edge.end.y, 120.00338856467053);
        let edge = result.edges[2].unwrap();
        close(edge.start.x, 42.87761760913126);
        close(edge.start.y, 211.0101656940116);
        close(edge.end.x, 56.83262975352574);
        close(edge.end.y, 52.0);
        let edge = result.edges[3].unwrap();
        close(edge.start.x, 55.252620670687016);
        close(edge.start.y, 70.00338856467053);
        close(edge.end.x, 56.83262975352574);
        close(edge.end.y, 52.0);
    }
    {
        // crowded
        let nodes = vec![
            Node {
                width: 30.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
            Node {
                width: 100.0,
                height: 80.0,
                ..Node::default()
            },
        ];
        let edges = vec![
            Edge::new(0, 1),
            Edge::new(0, 2),
            Edge::new(0, 3),
            Edge::new(0, 4),
            Edge::new(0, 5),
            Edge::new(0, 6),
            Edge::new(0, 7),
            Edge::new(0, 8),
        ];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 465.9508837899322);
        close(result.height, 445.9508837899322);
        let expected = [
            (217.97544189496617, 207.97544189496617),
            (353.9508837899322, 253.79578876059426),
            (253.7957887605943, 353.9508837899322),
            (112.15509502933756, 353.9508837899322),
            (12.0, 253.7957887605943),
            (12.0, 112.15509502933756),
            (112.15509502933747, 12.0),
            (253.7957887605943, 12.0),
            (353.95088378993216, 112.15509502933747),
        ];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        let edge = result.edges[0].unwrap();
        close(edge.start.x, 247.97544189496617);
        close(edge.start.y, 229.18864533056262);
        close(edge.end.x, 353.9508837899322);
        close(edge.end.y, 273.08511064193954);
        let edge = result.edges[1].unwrap();
        close(edge.start.x, 239.18864533056262);
        close(edge.start.y, 237.97544189496617);
        close(edge.end.x, 287.2272462656705);
        close(edge.end.y, 353.9508837899322);
        let edge = result.edges[2].unwrap();
        close(edge.start.x, 226.76223845936968);
        close(edge.start.y, 237.97544189496617);
        close(edge.end.x, 178.7236375242615);
        close(edge.end.y, 353.9508837899322);
        let edge = result.edges[3].unwrap();
        close(edge.start.x, 217.97544189496617);
        close(edge.start.y, 229.1886453305626);
        close(edge.end.x, 112.0);
        close(edge.end.y, 273.08511064193954);
        let edge = result.edges[4].unwrap();
        close(edge.start.x, 217.97544189496617);
        close(edge.start.y, 216.7622384593697);
        close(edge.end.x, 112.0);
        close(edge.end.y, 172.86577314799248);
        let edge = result.edges[5].unwrap();
        close(edge.start.x, 226.76223845936968);
        close(edge.start.y, 207.97544189496617);
        close(edge.end.x, 178.72363752426142);
        close(edge.end.y, 92.0);
        let edge = result.edges[6].unwrap();
        close(edge.start.x, 239.1886453305626);
        close(edge.start.y, 207.97544189496617);
        close(edge.end.x, 287.2272462656705);
        close(edge.end.y, 92.0);
        let edge = result.edges[7].unwrap();
        close(edge.start.x, 247.97544189496617);
        close(edge.start.y, 216.76223845936968);
        close(edge.end.x, 353.95088378993216);
        close(edge.end.y, 172.86577314799243);
    }
    {
        // ports
        let nodes = vec![
            Node {
                width: 40.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 60.0,
                height: 20.0,
                ..Node::default()
            },
        ];
        let edges = vec![Edge {
            source: 0,
            target: 1,
            source_is_port: true,
        }];
        let options = Options::default();
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 104.0);
        close(result.height, 59.0);
        let expected = [(12.0, 12.0), (32.0, 27.0)];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        assert!(result.edges[0].is_none());
    }
    {
        // container
        let nodes = vec![
            Node {
                width: 40.0,
                height: 30.0,
                ..Node::default()
            },
            Node {
                width: 60.0,
                height: 20.0,
                ..Node::default()
            },
            Node {
                width: 20.0,
                height: 50.0,
                ..Node::default()
            },
            Node {
                width: 30.0,
                height: 30.0,
                ..Node::default()
            },
        ];
        let edges = vec![Edge::new(0, 1), Edge::new(0, 2), Edge::new(0, 3)];
        let options = Options {
            spacing: 40.0,
            padding: Padding {
                top: 50.0,
                left: 10.0,
                bottom: 20.0,
                right: 30.0,
            },
        };
        let result = layout(&nodes, &edges, &options, &mut NoopWorkControl).unwrap();
        close(result.width, 165.42794885482215);
        close(result.height, 201.83548211372658);
        let expected = [
            (58.003719314851395, 96.90612697654616),
            (68.21441927407362, 161.83548211372658),
            (10.0, 61.69564214098749),
            (105.42794885482215, 50.0),
        ];
        for (point, (x, y)) in result.nodes.iter().zip(expected) {
            close(point.x, x);
            close(point.y, y);
        }
        let edge = result.edges[0].unwrap();
        close(edge.start.x, 83.06235040965274);
        close(edge.start.y, 126.90612697654616);
        close(edge.end.x, 94.84199854420606);
        close(edge.end.y, 161.83548211372658);
        let edge = result.edges[1].unwrap();
        close(edge.start.x, 58.003719314851395);
        close(edge.start.y, 103.2134137735486);
        close(edge.end.x, 30.0);
        close(edge.end.y, 91.04199874248627);
        let edge = result.edges[2].unwrap();
        close(edge.start.x, 91.57046387719296);
        close(edge.start.y, 96.90612697654616);
        close(edge.end.x, 106.86120429248056);
        close(edge.end.y, 80.0);
    }
}

#[test]
fn rejects_missing_roots_reachable_cycles_and_invalid_input_without_mutation() {
    let nodes = vec![
        Node {
            width: 40.0,
            height: 30.0,
            ..Node::default()
        };
        3
    ];
    let original = nodes.clone();
    assert_eq!(
        layout(
            &nodes,
            &[Edge::new(0, 1), Edge::new(1, 0), Edge::new(1, 2)],
            &Options::default(),
            &mut NoopWorkControl
        ),
        Err(Error::MissingRoot)
    );
    assert_eq!(
        layout(
            &nodes,
            &[Edge::new(0, 1), Edge::new(1, 2), Edge::new(2, 1)],
            &Options::default(),
            &mut NoopWorkControl
        ),
        Err(Error::ReachableCycle)
    );
    assert_eq!(
        layout(
            &nodes,
            &[Edge::new(0, 3)],
            &Options::default(),
            &mut NoopWorkControl
        ),
        Err(Error::InvalidEdge(0))
    );
    assert_eq!(
        layout(
            &nodes,
            &[],
            &Options {
                spacing: f64::NAN,
                ..Options::default()
            },
            &mut NoopWorkControl
        ),
        Err(Error::InvalidOption("spacing"))
    );
    assert_eq!(nodes, original);
    assert_eq!(
        layout(&[], &[], &Options::default(), &mut NoopWorkControl),
        Ok(Layout::default())
    );
}

#[test]
fn work_control_interrupts_radius_extension() {
    struct Budget {
        remaining: usize,
    }
    impl WorkControl for Budget {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            if units > self.remaining {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.check(units)?;
            self.remaining -= units;
            Ok(())
        }
    }
    let mut nodes = vec![
        Node {
            width: 100.0,
            height: 80.0,
            ..Node::default()
        };
        9
    ];
    nodes[0].width = 30.0;
    nodes[0].height = 30.0;
    let edges: Vec<_> = (1..9).map(|target| Edge::new(0, target)).collect();
    let mut work = Budget { remaining: 300 };
    assert_eq!(
        layout(&nodes, &edges, &Options::default(), &mut work),
        Err(Error::Work(WorkError::Interrupted))
    );
    // Import, wedge allocation, and initial placement cost less than this budget. The actual
    // crowded-source case requires many one-pixel radius extensions, each charged separately.
    assert!(work.remaining < 8);
}

#[test]
fn deep_trees_do_not_use_the_call_stack_and_numeric_failures_are_explicit() {
    let nodes = vec![
        Node {
            width: 40.0,
            height: 30.0,
            ..Node::default()
        };
        2048
    ];
    let edges: Vec<_> = (1..nodes.len())
        .map(|target| Edge::new(target - 1, target))
        .collect();
    let result = layout(&nodes, &edges, &Options::default(), &mut NoopWorkControl).unwrap();
    assert_eq!(result.nodes.len(), nodes.len());
    assert!(result.edges.iter().all(Option::is_some));
    let oversized = [Node {
        width: f64::MAX,
        ..Node::default()
    }];
    assert_eq!(
        layout(&oversized, &[], &Options::default(), &mut NoopWorkControl),
        Err(Error::NonFiniteGeometry)
    );
    let zero_nodes = [Node::default(), Node::default()];
    assert_eq!(
        layout(
            &zero_nodes,
            &[Edge::new(0, 1)],
            &Options::default(),
            &mut NoopWorkControl
        ),
        Err(Error::NonFiniteGeometry)
    );
}
