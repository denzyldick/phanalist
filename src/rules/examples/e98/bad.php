<?php
// Bad: session ID in URL
$sid = session_id() ?? '';
header("Location: /page?session=" . $sid);
