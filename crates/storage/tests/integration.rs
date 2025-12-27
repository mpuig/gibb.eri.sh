//! Integration tests for the storage crate.
//!
//! Uses in-memory SQLite for fast, isolated tests.

use gibberish_events::{Activity, ActivityContent, ActivityStatus, ActivityType};
use gibberish_storage::{ActivityRepository, Database, StorageError};
use gibberish_transcript::{Transcript, TranscriptRepository};
use uuid::Uuid;

fn create_test_db() -> Database {
    Database::open_in_memory().expect("Failed to create in-memory database")
}

fn create_test_transcript() -> Transcript {
    let mut transcript = Transcript::new();
    transcript.title = Some("Test Session".to_string());
    transcript.duration_ms = 5000;
    transcript
}

fn create_test_activity(activity_type: ActivityType) -> Activity {
    Activity {
        id: Uuid::new_v4().to_string(),
        activity_type,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        status: ActivityStatus::Completed,
        parent_id: None,
        content: ActivityContent {
            text: Some("Test activity".to_string()),
            ..Default::default()
        },
        expanded: None,
    }
}

// =============================================================================
// Database Initialization Tests
// =============================================================================

mod initialization {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory();
        assert!(db.is_ok(), "Should create in-memory database");
    }

    #[test]
    fn test_open_file_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = Database::open(&db_path);
        assert!(db.is_ok(), "Should create file-based database");
        assert!(db_path.exists(), "Database file should exist");
    }

    #[test]
    fn test_reopen_existing_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create and save a transcript
        {
            let db = Database::open(&db_path).unwrap();
            let transcript = create_test_transcript();
            db.save(&transcript).unwrap();
        }

        // Reopen and verify data persists
        {
            let db = Database::open(&db_path).unwrap();
            let transcripts = db.list().unwrap();
            assert_eq!(transcripts.len(), 1, "Transcript should persist after reopen");
        }
    }

    #[test]
    fn test_invalid_path_fails() {
        let result = Database::open(&PathBuf::from("/nonexistent/path/db.sqlite"));
        assert!(result.is_err(), "Should fail with invalid path");
    }
}

// =============================================================================
// Transcript Repository Tests
// =============================================================================

mod transcripts {
    use super::*;

    #[test]
    fn test_save_and_get_transcript() {
        let db = create_test_db();
        let transcript = create_test_transcript();
        let id = transcript.id;

        db.save(&transcript).unwrap();

        let retrieved = db.get(&id).unwrap();
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.title, transcript.title);
        assert_eq!(retrieved.duration_ms, transcript.duration_ms);
    }

    #[test]
    fn test_get_nonexistent_transcript() {
        let db = create_test_db();
        let fake_id = Uuid::new_v4();

        let result = db.get(&fake_id);
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_list_transcripts_empty() {
        let db = create_test_db();
        let transcripts = db.list().unwrap();
        assert!(transcripts.is_empty());
    }

    #[test]
    fn test_list_transcripts_ordered_by_created_at() {
        let db = create_test_db();

        // Save multiple transcripts
        let mut t1 = create_test_transcript();
        t1.title = Some("First".to_string());
        db.save(&t1).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut t2 = create_test_transcript();
        t2.title = Some("Second".to_string());
        db.save(&t2).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut t3 = create_test_transcript();
        t3.title = Some("Third".to_string());
        db.save(&t3).unwrap();

        // List should return newest first
        let transcripts = db.list().unwrap();
        assert_eq!(transcripts.len(), 3);
        assert_eq!(transcripts[0].title, Some("Third".to_string()));
        assert_eq!(transcripts[1].title, Some("Second".to_string()));
        assert_eq!(transcripts[2].title, Some("First".to_string()));
    }

    #[test]
    fn test_update_transcript() {
        let db = create_test_db();
        let mut transcript = create_test_transcript();
        let id = transcript.id;

        db.save(&transcript).unwrap();

        // Update the transcript
        transcript.title = Some("Updated Title".to_string());
        transcript.duration_ms = 10000;
        db.save(&transcript).unwrap();

        // Verify update
        let retrieved = db.get(&id).unwrap();
        assert_eq!(retrieved.title, Some("Updated Title".to_string()));
        assert_eq!(retrieved.duration_ms, 10000);

        // Should still be only one transcript
        let all = db.list().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_delete_transcript() {
        let db = create_test_db();
        let transcript = create_test_transcript();
        let id = transcript.id;

        db.save(&transcript).unwrap();
        assert!(db.get(&id).is_ok());

        db.delete(&id).unwrap();
        assert!(matches!(db.get(&id), Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_delete_nonexistent_transcript() {
        let db = create_test_db();
        let fake_id = Uuid::new_v4();

        let result = db.delete(&fake_id);
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_save_transcript_with_segments() {
        use gibberish_transcript::Segment;

        let db = create_test_db();
        let mut transcript = create_test_transcript();

        transcript.segments.push(Segment {
            id: Uuid::new_v4(),
            text: "Hello world".to_string(),
            start_ms: 0,
            end_ms: 1000,
            words: Vec::new(),
            is_final: true,
            speaker: None,
        });

        transcript.segments.push(Segment {
            id: Uuid::new_v4(),
            text: "How are you".to_string(),
            start_ms: 1000,
            end_ms: 2000,
            words: Vec::new(),
            is_final: true,
            speaker: None,
        });

        let id = transcript.id;
        db.save(&transcript).unwrap();

        let retrieved = db.get(&id).unwrap();
        assert_eq!(retrieved.segments.len(), 2);
        assert_eq!(retrieved.segments[0].text, "Hello world");
        assert_eq!(retrieved.segments[1].text, "How are you");
    }
}

// =============================================================================
// Activity Repository Tests
// =============================================================================

mod activities {
    use super::*;

    #[test]
    fn test_save_and_get_activity() {
        let db = create_test_db();
        let activity = create_test_activity(ActivityType::Transcript);

        db.save_activity(&activity).unwrap();

        let activities = db.get_activities(10).unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities[0].id, activity.id);
        assert_eq!(activities[0].activity_type, ActivityType::Transcript);
    }

    #[test]
    fn test_get_activities_empty() {
        let db = create_test_db();
        let activities = db.get_activities(10).unwrap();
        assert!(activities.is_empty());
    }

    #[test]
    fn test_get_activities_with_limit() {
        let db = create_test_db();

        // Save 5 activities
        for _ in 0..5 {
            let activity = create_test_activity(ActivityType::Transcript);
            std::thread::sleep(std::time::Duration::from_millis(5));
            db.save_activity(&activity).unwrap();
        }

        // Request only 3
        let activities = db.get_activities(3).unwrap();
        assert_eq!(activities.len(), 3);
    }

    #[test]
    fn test_get_activities_ordered_by_timestamp() {
        let db = create_test_db();

        let mut a1 = create_test_activity(ActivityType::Transcript);
        a1.content.text = Some("First".to_string());
        a1.timestamp = 1000;
        db.save_activity(&a1).unwrap();

        let mut a2 = create_test_activity(ActivityType::VoiceCommand);
        a2.content.text = Some("Second".to_string());
        a2.timestamp = 2000;
        db.save_activity(&a2).unwrap();

        let mut a3 = create_test_activity(ActivityType::ToolResult);
        a3.content.text = Some("Third".to_string());
        a3.timestamp = 3000;
        db.save_activity(&a3).unwrap();

        // Should be ordered newest first
        let activities = db.get_activities(10).unwrap();
        assert_eq!(activities.len(), 3);
        assert_eq!(activities[0].content.text, Some("Third".to_string()));
        assert_eq!(activities[1].content.text, Some("Second".to_string()));
        assert_eq!(activities[2].content.text, Some("First".to_string()));
    }

    #[test]
    fn test_save_activity_types() {
        let db = create_test_db();

        let types = [
            ActivityType::Transcript,
            ActivityType::VoiceCommand,
            ActivityType::ToolResult,
            ActivityType::ToolError,
        ];

        for activity_type in types {
            let activity = create_test_activity(activity_type.clone());
            db.save_activity(&activity).unwrap();

            let retrieved = db.get_activities(100).unwrap();
            let found = retrieved.iter().find(|a| a.id == activity.id);
            assert!(found.is_some(), "Activity should be found");
            assert_eq!(found.unwrap().activity_type, activity_type);
        }
    }

    #[test]
    fn test_save_activity_with_parent() {
        let db = create_test_db();

        let parent = create_test_activity(ActivityType::VoiceCommand);
        let parent_id = parent.id.clone();
        db.save_activity(&parent).unwrap();

        let mut child = create_test_activity(ActivityType::ToolResult);
        child.parent_id = Some(parent_id.clone());
        db.save_activity(&child).unwrap();

        let activities = db.get_activities(10).unwrap();
        let retrieved_child = activities.iter().find(|a| a.id == child.id).unwrap();
        assert_eq!(retrieved_child.parent_id, Some(parent_id));
    }

    #[test]
    fn test_delete_activity() {
        let db = create_test_db();
        let activity = create_test_activity(ActivityType::Transcript);
        let id = activity.id.clone();

        db.save_activity(&activity).unwrap();
        assert_eq!(db.get_activities(10).unwrap().len(), 1);

        db.delete_activity(&id).unwrap();
        assert!(db.get_activities(10).unwrap().is_empty());
    }

    #[test]
    fn test_delete_nonexistent_activity() {
        let db = create_test_db();

        let result = db.delete_activity("nonexistent-id");
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_clear_activities() {
        let db = create_test_db();

        // Save multiple activities
        for _ in 0..5 {
            let activity = create_test_activity(ActivityType::Transcript);
            db.save_activity(&activity).unwrap();
        }

        assert_eq!(db.get_activities(10).unwrap().len(), 5);

        db.clear_activities().unwrap();
        assert!(db.get_activities(10).unwrap().is_empty());
    }

    #[test]
    fn test_clear_activities_empty_db() {
        let db = create_test_db();
        // Should not error on empty database
        db.clear_activities().unwrap();
    }
}

// =============================================================================
// Concurrent Access Tests
// =============================================================================

mod concurrency {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_reads() {
        let db = Arc::new(create_test_db());

        // Save some data
        for _ in 0..10 {
            let transcript = create_test_transcript();
            db.save(&transcript).unwrap();
        }

        // Spawn multiple reader threads
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let db_clone = Arc::clone(&db);
                thread::spawn(move || {
                    for _ in 0..10 {
                        let transcripts = db_clone.list().unwrap();
                        assert_eq!(transcripts.len(), 10);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn test_concurrent_writes() {
        let db = Arc::new(create_test_db());

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let db_clone = Arc::clone(&db);
                thread::spawn(move || {
                    for j in 0..10 {
                        let mut transcript = create_test_transcript();
                        transcript.title = Some(format!("Thread {} Transcript {}", i, j));
                        db_clone.save(&transcript).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let transcripts = db.list().unwrap();
        assert_eq!(transcripts.len(), 50, "All 50 transcripts should be saved");
    }

    #[test]
    fn test_concurrent_mixed_operations() {
        let db = Arc::new(create_test_db());

        // Pre-populate with some data
        let mut ids = Vec::new();
        for _ in 0..10 {
            let transcript = create_test_transcript();
            ids.push(transcript.id);
            db.save(&transcript).unwrap();
        }

        let ids = Arc::new(ids);

        // Reader threads
        let reader_handles: Vec<_> = (0..3)
            .map(|_| {
                let db_clone = Arc::clone(&db);
                thread::spawn(move || {
                    for _ in 0..20 {
                        let _ = db_clone.list();
                    }
                })
            })
            .collect();

        // Writer threads
        let writer_handles: Vec<_> = (0..2)
            .map(|_| {
                let db_clone = Arc::clone(&db);
                thread::spawn(move || {
                    for _ in 0..10 {
                        let activity = create_test_activity(ActivityType::Transcript);
                        let _ = db_clone.save_activity(&activity);
                    }
                })
            })
            .collect();

        for handle in reader_handles {
            handle.join().expect("Reader thread panicked");
        }

        for handle in writer_handles {
            handle.join().expect("Writer thread panicked");
        }

        // Verify database is still consistent
        let transcripts = db.list().unwrap();
        assert!(transcripts.len() >= 10, "Original transcripts should exist");

        let activities = db.get_activities(100).unwrap();
        assert_eq!(activities.len(), 20, "All activities should be saved");
    }
}

// =============================================================================
// Edge Cases
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn test_save_transcript_with_empty_title() {
        let db = create_test_db();
        let mut transcript = create_test_transcript();
        transcript.title = None;

        let id = transcript.id;
        db.save(&transcript).unwrap();

        let retrieved = db.get(&id).unwrap();
        assert_eq!(retrieved.title, None);
    }

    #[test]
    fn test_save_transcript_with_long_title() {
        let db = create_test_db();
        let mut transcript = create_test_transcript();
        transcript.title = Some("x".repeat(10000));

        let id = transcript.id;
        db.save(&transcript).unwrap();

        let retrieved = db.get(&id).unwrap();
        assert_eq!(retrieved.title.as_ref().map(|s| s.len()), Some(10000));
    }

    #[test]
    fn test_save_activity_with_large_content() {
        let db = create_test_db();
        let mut activity = create_test_activity(ActivityType::Transcript);
        activity.content.text = Some("x".repeat(100000));

        db.save_activity(&activity).unwrap();

        let activities = db.get_activities(10).unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(
            activities[0].content.text.as_ref().map(|s| s.len()),
            Some(100000)
        );
    }

    #[test]
    fn test_save_activity_with_unicode() {
        let db = create_test_db();
        let mut activity = create_test_activity(ActivityType::Transcript);
        activity.content.text = Some("Hello 世界 🌍 مرحبا".to_string());

        db.save_activity(&activity).unwrap();

        let activities = db.get_activities(10).unwrap();
        assert_eq!(
            activities[0].content.text,
            Some("Hello 世界 🌍 مرحبا".to_string())
        );
    }

    #[test]
    fn test_many_activities() {
        let db = create_test_db();

        // Insert 1000 activities
        for i in 0..1000 {
            let mut activity = create_test_activity(ActivityType::Transcript);
            activity.timestamp = i;
            db.save_activity(&activity).unwrap();
        }

        // Verify we can retrieve them
        let activities = db.get_activities(1000).unwrap();
        assert_eq!(activities.len(), 1000);

        // Verify ordering (newest first)
        assert_eq!(activities[0].timestamp, 999);
        assert_eq!(activities[999].timestamp, 0);
    }
}
