<?php
// Bad: unsafe file upload to user-controlled destination
$dest = $_GET['dest'] ?? '';
move_uploaded_file($_FILES['file']['tmp_name'], $dest);
file_put_contents($_POST['path'], $content);
