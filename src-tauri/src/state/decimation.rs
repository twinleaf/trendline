use super::capture::Point;

#[derive(Clone, Copy, PartialEq)]
enum LastRetained {
    None,
    Max, 
    Min, 
}


pub fn lttb(data: Vec<Point>, threshold: usize) -> Vec<Point> {
    if threshold >= data.len() || threshold == 0 {
        return data; // Nothing to do
    }

    let mut sampled = Vec::with_capacity(threshold);
    let data_len = data.len();
    let every = (data_len - 2) as f64 / (threshold - 2) as f64;
    let mut a = 0;

    sampled.push(data[a]);

    for i in 0..threshold - 2 {
        let mut avg_x = 0.0;
        let mut avg_y = 0.0;
        let avg_range_start = (((i + 1) as f64) * every) as usize + 1;
        let mut avg_range_end = (((i + 2) as f64) * every) as usize + 1;
        if avg_range_end >= data_len {
            avg_range_end = data_len;
        }

        let avg_range_len = (avg_range_end - avg_range_start) as f64;

        for i in 0..(avg_range_end - avg_range_start) {
            let idx = (avg_range_start + i) as usize;
            avg_x += data[idx].x; 
            avg_y += data[idx].y;
        }
        avg_x /= avg_range_len;
        avg_y /= avg_range_len;

        let range_offs = ((i as f64) * every) as usize + 1;
        let range_to = (((i + 1) as f64) * every) as usize + 1;
        
        let point_a_x = data[a].x; 
        let point_a_y = data[a].y;

        let mut max_area = -1.0;
        let mut next_a = range_offs;

        for i in 0..(range_to - range_offs) {
            let idx = (range_offs + i) as usize;

            let area = ((point_a_x - avg_x) * (data[idx].y - point_a_y)
                - (point_a_x - data[idx].x) * (avg_y - point_a_y))
                .abs() * 0.5;

            if area > max_area {
                max_area = area;
                next_a = idx;
            }
        }
        sampled.push(data[next_a]);
        a = next_a;
    }

    sampled.push(data[data_len - 1]);
    sampled
}


pub fn fpcs(data: &[Point], ratio: usize) -> Vec<Point> {
    if data.len() <= 2 || ratio < 2 || data.len() <= ratio {
        return data.to_vec();
    }

    let mut retained_points = Vec::with_capacity(data.len() / ratio + 2);

    let mut potential_point: Option<Point> = None;
    let mut last_retained = LastRetained::None;
    let mut counter = 0;
    

    let mut max_point = data[0];
    let mut min_point = data[0];
    
    retained_points.push(data[0]);

    for p in &data[1..] {
        counter += 1;

        if p.y >= max_point.y {
            max_point = *p;
        } else if p.y < min_point.y {
            min_point = *p;
        }

        if counter >= ratio {
            if min_point.x < max_point.x {
                
                if last_retained == LastRetained::Min && Some(min_point) != potential_point {
                    if let Some(pp) = potential_point {
                        retained_points.push(pp);
                    }
                }
                
                retained_points.push(min_point);
                
                potential_point = Some(max_point);
                min_point = max_point;
                last_retained = LastRetained::Min;

            } else {
                
                if last_retained == LastRetained::Max && Some(max_point) != potential_point {
                     if let Some(pp) = potential_point {
                        retained_points.push(pp);
                    }
                }
                
                retained_points.push(max_point);
                
                potential_point = Some(min_point);
                max_point = min_point; 
                last_retained = LastRetained::Max;
            }

            counter = 0;
        }
    }
    
    if let Some(&last_point) = data.last() {
        if retained_points.last() != Some(&last_point) {
            retained_points.push(last_point);
        }
    }

    retained_points
}