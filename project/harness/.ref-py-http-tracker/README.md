Build and run with:
```
docker build -t simpletracker . && docker run -p 6969:6969 simpletracker
```

**Notes:**

This is just a hobbyist project, HTTP Trackers are kinda dead.

The tracker expects clients to signal that they finished a download, if a client
doesn't do this or doesn't shut down gracefully we will accumulate entries on
the DB eternally 🥶 

There is nothing done here to add `torrents` to the tracker, for testing purposes
I just hardcoded them with `shelve` (WIP)