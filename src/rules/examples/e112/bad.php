<?php
// Bad: file upload without validation
$dest = $_GET['dest'] ?? '';
move_uploaded_file($_FILES['file']['tmp_name'], $dest);
