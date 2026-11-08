<?php
// Bad: mail injection via user-controlled input
$to = $_GET['to'] ?? '';
$subject = $_POST['subject'] ?? '';
mail($to, $subject, $body);
mb_send_mail($to, $subject, $body, $headers);
