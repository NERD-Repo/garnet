#include "garnet/examples/ui/video_display/frame_scheduler.h"
#include "lib/fxl/command_line.h"

namespace video_display {

// Get the next frame presentation time
uint64_t FrameScheduler::EnqueBuffer(uint32_t buffer_id) {
    std::lock_guard<std::mutex> lck (time_lock_);
    int32_t prev_buffer = FindBuffer(buffer_id);
    for (uint32_t i = 0; i < buffers_.size(); ++i) {
        if (prev_buffer >= 0) {
            FXL_LOG(ERROR) << "Getting additional presentation times for "
                << "already enqued frames! Buffer " 
                << buffers_[prev_buffer].id
                << " previously enqued at time " 
                << buffers_[prev_buffer].requested_pres_time;
            return 0;
        }
    }
    last_presentation_time_ns_ += presentation_interval_ns_;
    buffers_.push_back({buffer_id, last_presentation_time_ns_});
    return last_presentation_time_ns_;
}

int32_t FrameScheduler::FindBuffer(uint32_t buffer_id) {
    for (uint32_t i = 0; i < buffers_.size(); ++i) {
        if (buffers_[i].id == buffer_id) {
            return i;
        }
    }
    return -1;
}

int32_t FrameScheduler::GetFirstUnpresented() {
    for (uint32_t i = 0; i < buffers_.size(); ++i) {
        if (buffers_[i].presented == false) {
            return i;
        }
    }
    return -1;
}

bool FrameScheduler::IsEnqueued(uint32_t buffer_id) { return FindBuffer(buffer_id) >= 0; }

int32_t FrameScheduler::Update(uint64_t presentation_time, uint64_t presentation_interval,
        uint32_t buffer_id) {
    std::lock_guard<std::mutex> lck (time_lock_);
    if (buffers_.size() == 0) {
        FXL_LOG(INFO) << "Attempting to update with no queued times!";
        return -1;
    } // todo: add lots of errors
    int32_t prev_buffer = FindBuffer(buffer_id);
    int32_t first_unpres = GetFirstUnpresented();
    // If out of order error, complain but continue, so we don't screw up queue
    if (prev_buffer != first_unpres) {
        FXL_LOG(ERROR) << "Presenting out of order. Presenting position "
            << prev_buffer << " instead of " << first_unpres;
    }

    int64_t diff = presentation_time - buffers_[prev_buffer].requested_pres_time;
    uint64_t updated_time = presentation_time + presentation_interval_ns_ * (buffers_.size() - (prev_buffer + 1));
    // FXL_LOG(INFO) << " Buffer Presented: " << buffer_id;
    // FXL_LOG(INFO) << "Presentation time: " << presentation_time;
    // FXL_LOG(INFO) << "        requested: " << buffers_[prev_buffer].requested_pres_time;
    // FXL_LOG(INFO) << "           latest: " << last_presentation_time_ns_;
    // FXL_LOG(INFO) << "  possible update: " << updated_time;
    if (diff > 0) {
        // we are behind - we need to advance our presentation timing
        if (updated_time > last_presentation_time_ns_) {
            FXL_LOG(INFO) << "Presentation times falling behind.  updating by " 
                << updated_time - last_presentation_time_ns_;
            last_presentation_time_ns_ = updated_time;
        } else {
            FXL_LOG(INFO) << "Presentation times falling behind.  no update "; 
        }
        // last_presentation_time_ns_ += diff;
    } // todo error if < 0
    buffers_[prev_buffer].presented = true;
    presentation_interval_ns_ = presentation_interval;
    return 0;
}

int32_t FrameScheduler::ReleaseBuffer(uint32_t buffer_id) {
    std::lock_guard<std::mutex> lck (time_lock_);
    // TODO(garratt): complain if not the first, or not presented
    int32_t prev_buffer = FindBuffer(buffer_id);
    if (prev_buffer < 0) {
        FXL_LOG(ERROR) << "Buffer " << buffer_id << "was not in queue.";
        return -1;
    }
    buffers_.erase(buffers_.begin() + prev_buffer);
    return 0;
}

}  // namespace video_display
