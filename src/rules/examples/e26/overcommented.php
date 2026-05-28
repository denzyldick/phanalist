<?php

namespace App;

/**
 * This class handles user registration.
 *
 * It provides methods to register new users,
 * validate input, and send welcome emails.
 *
 * The registration process has multiple steps:
 * 1. Validate input
 * 2. Check for duplicate users
 * 3. Create the user record
 * 4. Send welcome email
 *
 * This class depends on UserRepository and MailerService.
 *
 * @package App
 */
class Overcommented
{
    // The user repository for database operations
    private $repository;

    // The mailer service for sending emails
    private $mailer;

    /**
     * Constructor
     *
     * @param $repository The user repository
     * @param $mailer The mailer service
     */
    public function __construct($repository, $mailer)
    {
        // Assign the repository
        $this->repository = $repository;
        // Assign the mailer
        $this->mailer = $mailer;
    }

    /**
     * Register a new user
     *
     * @param string $name The user name
     * @param string $email The email address
     * @return bool True if successful
     */
    public function register(string $name, string $email): bool
    {
        // Validate the input data
        if (empty($name)) {
            // Name is required
            return false;
        }
        // Check if email is valid
        if (empty($email)) {
            // Email is required
            return false;
        }
        // Save to database
        return true;
    }
}
