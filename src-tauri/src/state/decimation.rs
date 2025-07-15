use super::capture::Point;
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

    // Always preserve the true start and end points.
    let mut downsampled = Vec::with_capacity(target_points);
    downsampled.push(data[0]);

    // The inner data to be decimated.
    let inner_data = &data[1..data_len - 1];
    let inner_target_points = target_points - 2;

    let num_buckets = (inner_target_points / 2).max(1);
    let time_span = max_time - min_time;
    let bucket_duration = time_span / num_buckets as f64;
    let mut search_from_idx = 0;

    for i in 0..num_buckets {
        let bucket_t_start = min_time + (i as f64 * bucket_duration);
        let bucket_t_end = bucket_t_start + bucket_duration;

        let start_idx = search_from_idx
            + inner_data[search_from_idx..]
                .binary_search_by(|p| {
                    p.x.partial_cmp(&bucket_t_start)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or_else(|x| x);

        let end_idx = start_idx
            + inner_data[start_idx..]
                .binary_search_by(|p| {
                    p.x.partial_cmp(&bucket_t_end)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or_else(|x| x);
        
        search_from_idx = end_idx;

        if start_idx >= end_idx {
            continue; // No data in this bucket
        }

        let bucket = &inner_data[start_idx..end_idx];
        let representative_time = bucket_t_start + bucket_duration / 2.0;

        let y_values: Vec<f64> = bucket.iter().map(|p| p.y).collect();
        let (min_y_idx, max_y_idx) = y_values.argminmax();

        let mut point_min = bucket[min_y_idx];
        let mut point_max = bucket[max_y_idx];

        point_min.x = representative_time;
        point_max.x = representative_time;

        if bucket[min_y_idx].x < bucket[max_y_idx].x {
            downsampled.push(point_min);
            downsampled.push(point_max);
        } else if bucket[max_y_idx].x < bucket[min_y_idx].x {
            downsampled.push(point_max);
            downsampled.push(point_min);
        } else {
            downsampled.push(point_min);
            if min_y_idx != max_y_idx {
                downsampled.push(point_max);
            }
        }
    }

    downsampled.push(data[data_len - 1]);
    downsampled
}



pub fn lerp(p1: &Point, p2: &Point, x: f64) -> f64 {
    if (p2.x - p1.x).abs() < 1e-9 { return p1.y; }
    p1.y + (p2.y - p1.y) * (x - p1.x) / (p2.x - p1.x)
}