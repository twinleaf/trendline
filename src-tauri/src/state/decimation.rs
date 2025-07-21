use crate::shared::Point;
use argminmax::ArgMinMax;

#[derive(Clone, Copy, PartialEq)]
enum LastRetained {
    None,
    Max, 
    Min, 
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

pub fn min_max_bucket(data: &[Point], target_points: usize, min_time: f64, max_time: f64) -> Vec<Point> {
    let data_len = data.len();
    if data_len <= target_points || target_points < 4 || min_time >= max_time {
        return data.to_vec();
    }
    
    let Some(first_point) = data.first() else { return vec![]; };
    let Some(last_point) = data.last() else { return vec![]; };

    let inner_target_points = target_points.saturating_sub(2);
    let num_buckets = (inner_target_points / 2).max(1);
    let final_capacity = 2 + num_buckets * 2;
    let mut downsampled = Vec::with_capacity(final_capacity);

    downsampled.push(Point::new(min_time, first_point.y));

    let time_span = max_time - min_time;
    let bucket_duration = time_span / num_buckets as f64;
    
    let mut search_from_idx = 0;
    let mut last_real_y_value = first_point.y;

    for i in 0..num_buckets {
        let bucket_t_start = min_time + (i as f64 * bucket_duration);
        let bucket_t_end = bucket_t_start + bucket_duration;

        let mut bucket_start_idx = search_from_idx;
        while bucket_start_idx < data_len && data[bucket_start_idx].x < bucket_t_start {
            bucket_start_idx += 1;
        }

        let mut bucket_end_idx = bucket_start_idx;
        while bucket_end_idx < data_len && data[bucket_end_idx].x < bucket_t_end {
            bucket_end_idx += 1;
        }
        
        search_from_idx = bucket_end_idx;
        
        let bucket_slice = &data[bucket_start_idx..bucket_end_idx];
        let representative_time = bucket_t_start + bucket_duration / 2.0;

        if bucket_slice.is_empty() {

            downsampled.push(Point::new(representative_time, last_real_y_value));
            downsampled.push(Point::new(representative_time, last_real_y_value));
            continue;
        }

        let (min_idx, max_idx) = bucket_slice.iter().map(|p| p.y).collect::<Vec<_>>().argminmax();
        
        let p_min = bucket_slice[min_idx];
        let p_max = bucket_slice[max_idx];
        
        let p_min_aligned = Point::new(representative_time, p_min.y);
        let p_max_aligned = Point::new(representative_time, p_max.y);

        if p_min.x <= p_max.x {
            downsampled.push(p_min_aligned);
            downsampled.push(p_max_aligned);
        } else {
            downsampled.push(p_max_aligned);
            downsampled.push(p_min_aligned);
        }
        
        if let Some(p) = bucket_slice.last() {
            last_real_y_value = p.y;
        }
    }

    downsampled.push(Point::new(max_time, last_point.y));

    downsampled
}



pub fn lerp(p1: &Point, p2: &Point, x: f64) -> f64 {
    if (p2.x - p1.x).abs() < 1e-9 { return p1.y; }
    p1.y + (p2.y - p1.y) * (x - p1.x) / (p2.x - p1.x)
}